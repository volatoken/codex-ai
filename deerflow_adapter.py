"""
DeerFlow API Adapter for Codex AI Rust Gateway.
Bridges the Rust Gateway HTTP API to DeerFlow's embedded Python client.

Two modes:
  - /api/chat/stream      → Full DeerFlow agent (research, planning, tools)
  - /api/chat/fast         → Direct LLM call via OpenAI-compatible API (fast chat)

Endpoints:
  GET  /api/health        - Health check
  POST /api/chat/thread   - Create a thread
  POST /api/chat/stream   - Send message via DeerFlow agent (SSE stream)
  POST /api/chat/fast     - Send message directly to LLM (fast, no agent)
"""

import json
import logging
import os
import sys
import uuid
import asyncio
from pathlib import Path

import httpx

# Add DeerFlow backend to Python path (for DeerFlowClient + community tools)
DEER_FLOW_BACKEND = os.path.join(os.path.expanduser("~"), "deer-flow", "backend")
DEER_FLOW_PACKAGES = os.path.join(DEER_FLOW_BACKEND, "packages", "harness")
if DEER_FLOW_BACKEND not in sys.path:
    sys.path.insert(0, DEER_FLOW_BACKEND)
if DEER_FLOW_PACKAGES not in sys.path:
    sys.path.insert(0, DEER_FLOW_PACKAGES)

# Set DeerFlow config and home paths BEFORE importing DeerFlow
os.environ.setdefault(
    "DEER_FLOW_CONFIG_PATH",
    os.path.join(os.path.expanduser("~"), "deer-flow", "config.yaml"),
)
os.environ.setdefault(
    "DEER_FLOW_HOME",
    os.path.join(os.path.expanduser("~"), ".deer-flow"),
)

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse, StreamingResponse

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)
logger = logging.getLogger("deerflow-adapter")

app = FastAPI(title="DeerFlow Adapter for Codex AI")

# --- LLM config from environment ---
LLM_BASE_URL = os.environ.get("LLM_BASE_URL", "http://localhost:8317/v1")
LLM_API_KEY = os.environ.get("LLM_API_KEY", "sk-local-cliproxyapi-001")
LLM_MODEL = os.environ.get("LLM_MODEL", "gpt-5.3-codex")
LLM_MODEL_FAST = os.environ.get("LLM_MODEL_FAST", "gpt-5.4-mini")

# Shared httpx client for direct LLM calls (connection pooling)
_http_client: httpx.AsyncClient | None = None

SYSTEM_PROMPT = """You are Codex AI, a highly intelligent and helpful AI assistant operating in a Telegram group.
You are knowledgeable about programming, technology, crypto, DeFi, and general topics.
Rules:
- Answer concisely but thoroughly. Use markdown formatting supported by Telegram.
- If the user speaks Vietnamese, respond in Vietnamese. Match the user's language.
- For code, use proper code blocks with language tags.
- Be proactive: if the user's question is ambiguous, make a reasonable assumption and answer.
- Never refuse to help unless the request is clearly harmful.
- You can use emoji sparingly for friendliness.
- When the user asks about current events, prices, news, or anything that requires up-to-date information, USE the web_search tool. Do NOT say you cannot search the web.
- When you need to read the content of a specific URL, USE the web_fetch tool.
- Always prefer using tools to get real data rather than saying you don't have access."""


# --- Tool definitions for function calling ---
TOOLS = [
    {
        "type": "function",
        "function": {
            "name": "web_search",
            "description": "Search the web for current information, news, prices, articles, and facts. Use this whenever the user asks about current events or needs up-to-date data.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query",
                    },
                },
                "required": ["query"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "web_fetch",
            "description": "Fetch and read the content of a specific web page URL. Use this to get detailed information from a URL found via web_search.",
            "parameters": {
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch",
                    },
                },
                "required": ["url"],
            },
        },
    },
]


# Lazy-load DeerFlow community search tools
_web_search_tool = None
_web_fetch_tool = None


def _get_web_search_tool():
    global _web_search_tool
    if _web_search_tool is None:
        try:
            from deerflow.community.ddg_search.tools import web_search_tool
            _web_search_tool = web_search_tool
            logger.info("DuckDuckGo web_search_tool loaded")
        except Exception as e:
            logger.warning(f"Failed to load web_search_tool: {e}")
    return _web_search_tool


def _get_web_fetch_tool():
    global _web_fetch_tool
    if _web_fetch_tool is None:
        try:
            from deerflow.community.jina_ai.tools import web_fetch_tool
            _web_fetch_tool = web_fetch_tool
            logger.info("Jina web_fetch_tool loaded")
        except Exception as e:
            logger.warning(f"Failed to load web_fetch_tool: {e}")
    return _web_fetch_tool


def _execute_tool(name: str, arguments: dict) -> str:
    """Execute a tool by name and return its result as a string."""
    if name == "web_search":
        tool = _get_web_search_tool()
        if tool:
            try:
                return tool.invoke({"query": arguments.get("query", ""), "max_results": 3})
            except Exception as e:
                return f"Search error: {e}"
        return "Web search tool not available"
    elif name == "web_fetch":
        tool = _get_web_fetch_tool()
        if tool:
            try:
                result = tool.invoke({"url": arguments.get("url", "")})
                # Truncate to avoid token overflow
                if isinstance(result, str) and len(result) > 4000:
                    result = result[:4000] + "\n\n... (truncated)"
                return result
            except Exception as e:
                return f"Fetch error: {e}"
        return "Web fetch tool not available"
    return f"Unknown tool: {name}"


def get_http_client() -> httpx.AsyncClient:
    global _http_client
    if _http_client is None:
        _http_client = httpx.AsyncClient(timeout=180.0)
    return _http_client


# Lazy-initialized DeerFlow client
_client = None
_client_init_failed = False


def get_client():
    global _client, _client_init_failed
    if _client_init_failed:
        return None
    if _client is None:
        try:
            from deerflow.client import DeerFlowClient
            config_path = os.environ.get("DEER_FLOW_CONFIG_PATH")
            _client = DeerFlowClient(config_path=config_path)
            logger.info("DeerFlowClient initialized successfully")
        except Exception as e:
            logger.error(f"Failed to init DeerFlowClient: {e}. Using direct LLM fallback.")
            _client_init_failed = True
            return None
    return _client


@app.on_event("shutdown")
async def shutdown():
    global _http_client
    if _http_client:
        await _http_client.aclose()


@app.get("/api/health")
async def health():
    return {"status": "ok", "deerflow_available": _client is not None and not _client_init_failed}


@app.post("/api/chat/thread")
async def create_thread():
    thread_id = str(uuid.uuid4())
    return {"thread_id": thread_id, "id": thread_id}


async def _direct_llm_stream(messages: list[dict], model: str | None = None):
    """Call the LLM directly via OpenAI-compatible API (streaming, no tools)."""
    client = get_http_client()
    use_model = model or LLM_MODEL
    payload = {
        "model": use_model,
        "messages": messages,
        "stream": True,
        "temperature": 0.3,
        "max_tokens": 4096,
    }
    headers = {
        "Authorization": f"Bearer {LLM_API_KEY}",
        "Content-Type": "application/json",
    }
    async with client.stream(
        "POST",
        f"{LLM_BASE_URL}/chat/completions",
        json=payload,
        headers=headers,
        timeout=120.0,
    ) as response:
        async for line in response.aiter_lines():
            if line.startswith("data: "):
                data = line[6:]
                if data.strip() == "[DONE]":
                    break
                try:
                    chunk = json.loads(data)
                    delta = chunk.get("choices", [{}])[0].get("delta", {})
                    content = delta.get("content", "")
                    if content:
                        yield content
                except json.JSONDecodeError:
                    continue


async def _llm_call_with_tools(messages: list[dict], model: str | None = None) -> str:
    """Call LLM with tool support. Runs a loop: if the LLM requests tool calls,
    execute them and feed results back until we get a final text response."""
    client = get_http_client()
    use_model = model or LLM_MODEL
    headers = {
        "Authorization": f"Bearer {LLM_API_KEY}",
        "Content-Type": "application/json",
    }
    conversation = list(messages)
    max_rounds = 3  # prevent infinite tool loops

    for _ in range(max_rounds):
        payload = {
            "model": use_model,
            "messages": conversation,
            "tools": TOOLS,
            "temperature": 0.3,
            "max_tokens": 4096,
        }
        resp = await client.post(
            f"{LLM_BASE_URL}/chat/completions",
            json=payload,
            headers=headers,
            timeout=120.0,
        )
        resp.raise_for_status()
        data = resp.json()
        choice = data["choices"][0]
        msg = choice["message"]

        # If the LLM wants to call tools
        tool_calls = msg.get("tool_calls")
        if tool_calls:
            # Add assistant message with tool_calls to conversation (clean format)
            assistant_msg = {"role": "assistant", "tool_calls": tool_calls}
            if msg.get("content"):
                assistant_msg["content"] = msg["content"]
            conversation.append(assistant_msg)
            for tc in tool_calls:
                fn = tc["function"]
                tool_name = fn["name"]
                try:
                    tool_args = json.loads(fn["arguments"]) if isinstance(fn["arguments"], str) else fn["arguments"]
                except json.JSONDecodeError:
                    tool_args = {}

                logger.info(f"Tool call: {tool_name}({tool_args})")
                # Run tool in thread pool with timeout
                try:
                    tool_result = await asyncio.wait_for(
                        asyncio.to_thread(_execute_tool, tool_name, tool_args),
                        timeout=15.0,
                    )
                except asyncio.TimeoutError:
                    tool_result = f"Tool {tool_name} timed out after 15s"
                logger.info(f"Tool result: {str(tool_result)[:200]}")

                conversation.append({
                    "role": "tool",
                    "tool_call_id": tc["id"],
                    "content": str(tool_result),
                })
            continue  # Loop back for LLM to process tool results

        # No tool calls — return the final text
        content = msg.get("content") or ""
        return content

    # If we exhausted rounds, return whatever we have
    return msg.get("content") or "No response after tool calls."


async def _direct_llm_call(messages: list[dict], model: str | None = None, max_tokens: int = 4096) -> str:
    """Call the LLM directly (non-streaming, no tools) and return the full response."""
    client = get_http_client()
    use_model = model or LLM_MODEL
    payload = {
        "model": use_model,
        "messages": messages,
        "temperature": 0.3,
        "max_tokens": max_tokens,
    }
    headers = {
        "Authorization": f"Bearer {LLM_API_KEY}",
        "Content-Type": "application/json",
    }
    resp = await client.post(
        f"{LLM_BASE_URL}/chat/completions",
        json=payload,
        headers=headers,
        timeout=120.0,
    )
    resp.raise_for_status()
    data = resp.json()
    return data["choices"][0]["message"]["content"]


@app.post("/api/chat/fast")
async def chat_fast(request: Request):
    """Fast direct LLM endpoint with web search/fetch tool support.
    Returns JSON directly (not SSE) to avoid streaming timeout issues."""
    body = await request.json()
    messages = body.get("messages", [])
    thread_id = body.get("thread_id", str(uuid.uuid4()))
    model = body.get("model", None)

    user_message = ""
    for msg in messages:
        if msg.get("role") == "user":
            user_message = msg.get("content", "")

    if not user_message:
        return JSONResponse(status_code=400, content={"error": "No user message provided"})

    full_messages = [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": user_message},
    ]

    try:
        # Use tool-calling LLM flow (handles web_search, web_fetch automatically)
        result = await asyncio.wait_for(
            _llm_call_with_tools(full_messages, model),
            timeout=90.0,
        )
    except asyncio.TimeoutError:
        logger.warning("Tool-calling flow timed out, trying direct LLM")
        try:
            result = await asyncio.wait_for(
                _direct_llm_call(full_messages, model),
                timeout=30.0,
            )
        except Exception:
            return JSONResponse(status_code=504, content={"error": "LLM request timed out"})
    except Exception as e:
        logger.exception("Error in tool-calling LLM flow, falling back")
        try:
            result = await asyncio.wait_for(
                _direct_llm_call(full_messages, model),
                timeout=30.0,
            )
        except Exception as e2:
            return JSONResponse(status_code=500, content={"error": str(e2)})

    if not result:
        result = "Không có phản hồi từ AI."

    return JSONResponse(content={
        "node": "final_answer",
        "content": result,
        "thread_id": thread_id,
    })


BUILD_SYSTEM_PROMPT = """You are an expert code generator AI. Your job is to generate complete, production-ready source code.

CRITICAL OUTPUT FORMAT — you MUST output EVERY file using this EXACT format with triple backticks:
```filename: main.py
print("hello world")
```

```filename: requirements.txt
requests>=2.28
```

Rules:
- Each file MUST start with ```filename: <path> and end with ```
- Generate ALL files needed: source code, requirements.txt (for Python), README.md
- For Python projects: always include a main.py as the single entry point
- Include proper error handling and logging
- Make the code immediately runnable — no placeholders or TODOs
- Keep it simple and focused on the core functionality
- If the user speaks Vietnamese, add Vietnamese comments
- Do NOT include Dockerfile unless specifically requested"""


@app.post("/api/build/generate")
async def build_generate(request: Request):
    """Generate complete source code for a project. No web tools, optimized for code generation."""
    body = await request.json()
    plan = body.get("plan", "")
    project_name = body.get("project_name", "tool")

    if not plan:
        return JSONResponse(status_code=400, content={"error": "No plan provided"})

    messages = [
        {"role": "system", "content": BUILD_SYSTEM_PROMPT},
        {"role": "user", "content": f"Generate complete source code for project '{project_name}':\n\n{plan}"},
    ]

    try:
        result = await asyncio.wait_for(
            _direct_llm_call(messages, max_tokens=8192),
            timeout=180.0,
        )
    except asyncio.TimeoutError:
        logger.warning("Code generation timed out")
        return JSONResponse(status_code=504, content={"error": "Code generation timed out"})
    except Exception as e:
        logger.exception("Error in code generation")
        return JSONResponse(status_code=500, content={"error": str(e)})

    return JSONResponse(content={
        "content": result,
        "project_name": project_name,
    })


@app.post("/api/chat/stream")
async def chat_stream(request: Request):
    """Full DeerFlow agent endpoint - uses research/planning/tools."""
    body = await request.json()
    messages = body.get("messages", [])
    thread_id = body.get("thread_id", str(uuid.uuid4()))
    use_fast = body.get("fast", False)

    user_message = ""
    for msg in messages:
        if msg.get("role") == "user":
            user_message = msg.get("content", "")

    if not user_message:
        return JSONResponse(status_code=400, content={"error": "No user message provided"})

    # If fast mode requested OR DeerFlow is unavailable, use direct LLM
    client = get_client()
    if use_fast or client is None:
        full_messages = [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": user_message},
        ]

        async def generate_fast():
            collected = []
            try:
                async for chunk in _direct_llm_stream(full_messages):
                    collected.append(chunk)
                # Send the full response as one SSE event
                full_text = "".join(collected)
                sse_data = json.dumps({
                    "node": "final_answer",
                    "content": full_text,
                    "thread_id": thread_id,
                })
                yield f"data: {sse_data}\n\n"
            except Exception as e:
                logger.exception("Error in fast LLM fallback")
                try:
                    result = await _direct_llm_call(full_messages)
                    sse_data = json.dumps({
                        "node": "final_answer",
                        "content": result,
                        "thread_id": thread_id,
                    })
                    yield f"data: {sse_data}\n\n"
                except Exception as e2:
                    error_data = json.dumps({"error": str(e2)})
                    yield f"data: {error_data}\n\n"
            yield "data: [DONE]\n\n"

        return StreamingResponse(
            generate_fast(),
            media_type="text/event-stream",
            headers={"Cache-Control": "no-cache", "Connection": "keep-alive"},
        )

    # Full DeerFlow agent path
    async def generate():
        try:
            for event in client.stream(user_message, thread_id=thread_id):
                if event.type == "messages-tuple":
                    data = event.data
                    msgs = data.get("messages", [])
                    for m in msgs if isinstance(msgs, list) else []:
                        content = m.get("content", "") if isinstance(m, dict) else str(m)
                        if content:
                            sse_data = json.dumps({
                                "node": "agent",
                                "content": content,
                                "thread_id": thread_id,
                            })
                            yield f"data: {sse_data}\n\n"
                elif event.type == "values":
                    data = event.data
                    msgs = data.get("messages", [])
                    if msgs:
                        last = msgs[-1] if isinstance(msgs, list) else msgs
                        content = ""
                        if isinstance(last, dict):
                            content = last.get("content", "")
                        elif isinstance(last, str):
                            content = last
                        if content:
                            sse_data = json.dumps({
                                "node": "final_answer",
                                "content": content,
                                "thread_id": thread_id,
                            })
                            yield f"data: {sse_data}\n\n"
        except Exception as e:
            logger.exception("Error in DeerFlow stream, falling back to direct LLM")
            # Fallback to direct LLM on DeerFlow failure
            try:
                full_messages = [
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": user_message},
                ]
                result = await _direct_llm_call(full_messages)
                sse_data = json.dumps({
                    "node": "final_answer",
                    "content": result,
                    "thread_id": thread_id,
                })
                yield f"data: {sse_data}\n\n"
            except Exception as e2:
                error_data = json.dumps({"error": str(e2)})
                yield f"data: {error_data}\n\n"
        yield "data: [DONE]\n\n"

    return StreamingResponse(
        generate(),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "Connection": "keep-alive"},
    )


if __name__ == "__main__":
    import uvicorn
    port = int(os.environ.get("ADAPTER_PORT", "2024"))
    logger.info(f"Starting DeerFlow adapter on port {port}")
    uvicorn.run(app, host="0.0.0.0", port=port, log_level="info")
