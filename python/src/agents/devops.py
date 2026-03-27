"""
DevOps Agent — Generates Dockerfiles, deployment configs, and scripts.
"""

from pathlib import Path
from llm_client import LLMClient

PROMPT_PATH = Path(__file__).parent.parent.parent / "prompts" / "devops.md"


class DevOpsAgent:
    def __init__(self, llm: LLMClient):
        self.llm = llm
        self.system_prompt = self._load_prompt()

    def _load_prompt(self) -> str:
        if PROMPT_PATH.exists():
            return PROMPT_PATH.read_text(encoding="utf-8")
        return (
            "You are a DevOps engineer. Generate efficient Dockerfiles and "
            "deployment configurations. Optimize for small image size and security. "
            "Use multi-stage builds when appropriate."
        )

    async def generate_config(self, project_name: str, plan: dict) -> dict:
        """Generate deployment configuration."""
        deps = plan.get("dependencies", [])
        entry = plan.get("entry_point", "main.py")
        tool_type = plan.get("tool_type", "general")

        messages = [
            {"role": "system", "content": self.system_prompt},
            {
                "role": "user",
                "content": (
                    f"Generate deployment config for:\n"
                    f"Project: {project_name}\n"
                    f"Type: {tool_type}\n"
                    f"Entry: {entry}\n"
                    f"Dependencies: {deps}\n\n"
                    "Respond with JSON: {\n"
                    "  \"Dockerfile\": \"dockerfile content\",\n"
                    "  \"docker-compose.yml\": \"compose content (if needed)\",\n"
                    "  \".dockerignore\": \"ignore content\",\n"
                    "  \"deploy_strategy\": \"docker|direct\",\n"
                    "  \"memory_limit\": \"256m\",\n"
                    "  \"restart_policy\": \"unless-stopped\"\n"
                    "}"
                ),
            },
        ]
        return await self.llm.chat_json(messages)
