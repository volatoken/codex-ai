"""
Critic Agent — Reviews plans and code for quality, security, and feasibility.
"""

from pathlib import Path
from llm_client import LLMClient

PROMPT_PATH = Path(__file__).parent.parent.parent / "prompts" / "critic.md"


class CriticAgent:
    def __init__(self, llm: LLMClient):
        self.llm = llm
        self.system_prompt = self._load_prompt()

    def _load_prompt(self) -> str:
        if PROMPT_PATH.exists():
            return PROMPT_PATH.read_text(encoding="utf-8")
        return (
            "You are a critical code reviewer and security expert. "
            "Review plans and code for feasibility, security issues, and quality. "
            "Be thorough but constructive."
        )

    async def review(self, plan: dict) -> dict:
        """Review a plan and provide feedback."""
        messages = [
            {"role": "system", "content": self.system_prompt},
            {
                "role": "user",
                "content": (
                    f"Review this plan:\n{plan}\n\n"
                    "Respond with JSON: {\"approved\": true/false, "
                    "\"score\": 1-10, \"issues\": [...], \"suggestions\": [...], "
                    "\"security_concerns\": [...]}"
                ),
            },
        ]
        return await self.llm.chat_json(messages)

    async def review_code(self, filename: str, code: str) -> dict:
        """Review generated code."""
        messages = [
            {"role": "system", "content": self.system_prompt},
            {
                "role": "user",
                "content": (
                    f"Review this code ({filename}):\n```\n{code}\n```\n\n"
                    "Respond with JSON: {\"approved\": true/false, "
                    "\"issues\": [...], \"security_concerns\": [...], "
                    "\"suggested_fixes\": [...]}"
                ),
            },
        ]
        return await self.llm.chat_json(messages)
