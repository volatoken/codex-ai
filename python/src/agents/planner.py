"""
Planner Agent — Analyzes ideas and creates implementation plans.
"""

from pathlib import Path
from llm_client import LLMClient

PROMPT_PATH = Path(__file__).parent.parent.parent / "prompts" / "planner.md"


class PlannerAgent:
    def __init__(self, llm: LLMClient):
        self.llm = llm
        self.system_prompt = self._load_prompt()

    def _load_prompt(self) -> str:
        if PROMPT_PATH.exists():
            return PROMPT_PATH.read_text(encoding="utf-8")
        return (
            "You are a senior software architect. Analyze ideas and create "
            "detailed implementation plans. Respond in JSON format."
        )

    async def analyze(self, idea: str) -> dict:
        """Analyze an idea and produce a high-level plan."""
        messages = [
            {"role": "system", "content": self.system_prompt},
            {
                "role": "user",
                "content": (
                    f"Analyze this idea and create an implementation plan:\n\n{idea}\n\n"
                    "Respond with JSON: {\"project_name\": \"...\", \"summary\": \"...\", "
                    "\"tool_type\": \"simple-script|api-service|bot|cron-job\", "
                    "\"has_external_deps\": true/false, \"tech_stack\": [...], "
                    "\"estimated_files\": 5, \"steps\": [...]}"
                ),
            },
        ]
        return await self.llm.chat_json(messages)

    async def detail(self, project_name: str, plan: dict) -> dict:
        """Create a detailed plan with file structure and code outline."""
        messages = [
            {"role": "system", "content": self.system_prompt},
            {
                "role": "user",
                "content": (
                    f"Project: {project_name}\nPlan: {plan}\n\n"
                    "Generate a detailed implementation plan with file structure, "
                    "dependencies, and code outline for each file. "
                    "Respond with JSON: {\"files\": {\"filename\": \"description\"}, "
                    "\"dependencies\": [...], \"dockerfile_needed\": true/false, "
                    "\"env_vars\": [...], \"entry_point\": \"main.py\"}"
                ),
            },
        ]
        return await self.llm.chat_json(messages)
