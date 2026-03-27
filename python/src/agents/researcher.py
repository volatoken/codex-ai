"""
Researcher Agent — Analyzes queries and provides research summaries.
"""

from pathlib import Path
from llm_client import LLMClient

PROMPT_PATH = Path(__file__).parent.parent.parent / "prompts" / "researcher.md"


class ResearcherAgent:
    def __init__(self, llm: LLMClient):
        self.llm = llm
        self.system_prompt = self._load_prompt()

    def _load_prompt(self) -> str:
        if PROMPT_PATH.exists():
            return PROMPT_PATH.read_text(encoding="utf-8")
        return (
            "You are a technical researcher. Provide thorough, well-structured "
            "analysis of technical topics. Include pros/cons, alternatives, "
            "and practical recommendations."
        )

    async def research(self, query: str) -> str:
        """Research a topic and return a summary."""
        messages = [
            {"role": "system", "content": self.system_prompt},
            {
                "role": "user",
                "content": (
                    f"Research this topic and provide a comprehensive summary:\n\n"
                    f"{query}\n\n"
                    "Structure your response with sections: Overview, Key Points, "
                    "Pros/Cons (if applicable), Recommendations."
                ),
            },
        ]
        return await self.llm.chat(messages, temperature=0.5, max_tokens=4096)
