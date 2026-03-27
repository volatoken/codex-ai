"""
Coder Agent — Generates implementation code from plans.
"""

from pathlib import Path
from llm_client import LLMClient

PROMPT_PATH = Path(__file__).parent.parent.parent / "prompts" / "coder.md"


class CoderAgent:
    def __init__(self, llm: LLMClient):
        self.llm = llm
        self.system_prompt = self._load_prompt()

    def _load_prompt(self) -> str:
        if PROMPT_PATH.exists():
            return PROMPT_PATH.read_text(encoding="utf-8")
        return (
            "You are an expert Python developer. Generate clean, production-ready code. "
            "Follow best practices: proper error handling, logging, type hints. "
            "Always respond with JSON containing the file contents."
        )

    async def generate(self, project_name: str, plan: dict) -> dict:
        """Generate all code files for a project."""
        files_info = plan.get("files", {})
        deps = plan.get("dependencies", [])
        entry = plan.get("entry_point", "main.py")

        messages = [
            {"role": "system", "content": self.system_prompt},
            {
                "role": "user",
                "content": (
                    f"Project: {project_name}\n"
                    f"Files to generate: {files_info}\n"
                    f"Dependencies: {deps}\n"
                    f"Entry point: {entry}\n\n"
                    "Generate the complete code for ALL files. "
                    "Respond with JSON: {\"filename.py\": \"full code content\", ...}\n"
                    "Include a requirements.txt if there are dependencies.\n"
                    "Include a Dockerfile if the plan requires one."
                ),
            },
        ]
        return await self.llm.chat_json(messages, max_tokens=8192)

    async def fix(self, filename: str, code: str, error: str) -> str:
        """Fix code based on test failure or error."""
        messages = [
            {"role": "system", "content": self.system_prompt},
            {
                "role": "user",
                "content": (
                    f"Fix this code ({filename}):\n```python\n{code}\n```\n\n"
                    f"Error:\n{error}\n\n"
                    "Return ONLY the fixed code, no explanations."
                ),
            },
        ]
        return await self.llm.chat(messages, temperature=0.2)
