"""
LLM Client — async HTTP client for OpenRouter/OpenAI-compatible APIs.
"""

import json
from typing import Optional
import httpx


class LLMClient:
    def __init__(self, api_key: str, base_url: str, model: str):
        self.api_key = api_key
        self.base_url = base_url.rstrip("/")
        self.model = model

    async def chat(
        self,
        messages: list[dict],
        temperature: float = 0.7,
        max_tokens: int = 4096,
        json_mode: bool = False,
    ) -> str:
        """Send a chat completion request and return the response text."""
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }

        body = {
            "model": self.model,
            "messages": messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
        }

        if json_mode:
            body["response_format"] = {"type": "json_object"}

        async with httpx.AsyncClient(timeout=120) as client:
            resp = await client.post(
                f"{self.base_url}/chat/completions",
                headers=headers,
                json=body,
            )
            resp.raise_for_status()
            data = resp.json()

        return data["choices"][0]["message"]["content"]

    async def chat_json(
        self,
        messages: list[dict],
        temperature: float = 0.3,
        max_tokens: int = 4096,
    ) -> dict:
        """Chat and parse the result as JSON."""
        text = await self.chat(
            messages, temperature=temperature, max_tokens=max_tokens, json_mode=True
        )
        # Try to extract JSON from response
        text = text.strip()
        if text.startswith("```"):
            # Strip markdown code fences
            lines = text.split("\n")
            text = "\n".join(lines[1:-1])
        return json.loads(text)
