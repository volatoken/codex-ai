"""
Codex AI Python Worker
Reads JSON requests from stdin, processes them, writes JSON responses to stdout.

Protocol:
  - Input (stdin):  Single JSON line with {"action": "...", "payload": {...}}
  - Output (stdout): JSON lines. Updates: {"type":"update","message":"..."}, Final: {"status":"ok","result":{...}}
"""

import json
import sys
import asyncio
import os
from pathlib import Path

# Ensure project root is importable
sys.path.insert(0, str(Path(__file__).parent))

from llm_client import LLMClient
from agents.planner import PlannerAgent
from agents.critic import CriticAgent
from agents.coder import CoderAgent
from agents.tester import TesterAgent
from agents.devops import DevOpsAgent
from agents.researcher import ResearcherAgent


def send_update(message: str):
    """Send a streaming update to Rust."""
    print(json.dumps({"type": "update", "message": message}), flush=True)


def send_response(result: dict):
    """Send the final response to Rust."""
    print(json.dumps({"status": "ok", "result": result}), flush=True)


def send_error(error: str):
    """Send an error response to Rust."""
    print(json.dumps({"status": "error", "error": error}), flush=True)


async def process_request(action: str, payload: dict, llm: LLMClient) -> dict:
    """Dispatch action to the appropriate agent."""
    if action == "process_idea":
        send_update("🧠 Planner analyzing idea...")
        planner = PlannerAgent(llm)
        plan = await planner.analyze(payload["idea"])

        send_update("🔍 Critic reviewing plan...")
        critic = CriticAgent(llm)
        review = await critic.review(plan)

        return {
            "project_name": plan.get("project_name", "unnamed"),
            "plan_summary": plan.get("summary", ""),
            "plan": plan,
            "critique": review,
            "plan_ready": review.get("approved", False),
        }

    elif action == "plan":
        planner = PlannerAgent(llm)
        send_update("📋 Generating detailed plan...")
        plan = await planner.detail(payload["project_name"], payload.get("plan", {}))
        return {"plan": plan}

    elif action == "code":
        coder = CoderAgent(llm)
        send_update("💻 Generating code...")
        files = await coder.generate(payload["project_name"], payload.get("plan", {}))
        return {"files": files}

    elif action == "test":
        tester = TesterAgent(llm)
        send_update("🧪 Generating and running tests...")
        result = await tester.test(payload["project_name"])
        return result

    elif action == "deploy":
        devops = DevOpsAgent(llm)
        send_update("🐳 Generating deployment config...")
        config = await devops.generate_config(payload["project_name"], payload.get("plan", {}))
        return config

    elif action == "research":
        researcher = ResearcherAgent(llm)
        send_update("🔍 Researching...")
        answer = await researcher.research(payload["query"])
        return {"answer": answer}

    else:
        return {"error": f"Unknown action: {action}"}


def main():
    """Main entry point: read request from stdin, process, write response to stdout."""
    try:
        raw = sys.stdin.readline().strip()
        if not raw:
            send_error("Empty input")
            return

        request = json.loads(raw)
        action = request.get("action", "")
        payload = request.get("payload", {})

        llm = LLMClient(
            api_key=os.environ.get("LLM_API_KEY", ""),
            base_url=os.environ.get("LLM_BASE_URL", "https://openrouter.ai/api/v1"),
            model=os.environ.get("LLM_MODEL", "anthropic/claude-sonnet-4-20250514"),
        )

        result = asyncio.run(process_request(action, payload, llm))
        send_response(result)

    except json.JSONDecodeError as e:
        send_error(f"Invalid JSON: {e}")
    except Exception as e:
        send_error(f"Worker error: {e}")


if __name__ == "__main__":
    main()
