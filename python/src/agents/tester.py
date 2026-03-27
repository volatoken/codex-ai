"""
Tester Agent — Generates and runs tests for projects.
"""

import subprocess
import sys
from pathlib import Path
from llm_client import LLMClient

PROMPT_PATH = Path(__file__).parent.parent.parent / "prompts" / "tester.md"


class TesterAgent:
    def __init__(self, llm: LLMClient):
        self.llm = llm
        self.system_prompt = self._load_prompt()

    def _load_prompt(self) -> str:
        if PROMPT_PATH.exists():
            return PROMPT_PATH.read_text(encoding="utf-8")
        return (
            "You are a QA engineer. Generate comprehensive test cases. "
            "Use pytest for Python projects. Cover edge cases and error paths."
        )

    async def test(self, project_name: str) -> dict:
        """Generate tests and run them."""
        project_dir = Path(f"workspace/projects/{project_name}")
        src_dir = project_dir / "src"

        if not src_dir.exists():
            return {"passed": False, "error": "No source code found"}

        # Read all source files
        source_files = {}
        for f in src_dir.rglob("*.py"):
            source_files[f.name] = f.read_text(encoding="utf-8")

        if not source_files:
            return {"passed": False, "error": "No Python files found"}

        # Generate tests
        messages = [
            {"role": "system", "content": self.system_prompt},
            {
                "role": "user",
                "content": (
                    f"Generate pytest tests for these files:\n\n"
                    + "\n\n".join(
                        f"=== {name} ===\n{code}"
                        for name, code in source_files.items()
                    )
                    + "\n\nRespond with JSON: {\"test_main.py\": \"test code\"}"
                ),
            },
        ]

        test_files = await self.llm.chat_json(messages)

        # Write test files
        test_dir = project_dir / "tests"
        test_dir.mkdir(parents=True, exist_ok=True)
        for name, code in test_files.items():
            (test_dir / name).write_text(code, encoding="utf-8")

        # Run pytest
        try:
            result = subprocess.run(
                [sys.executable, "-m", "pytest", str(test_dir), "-v", "--tb=short"],
                capture_output=True,
                text=True,
                timeout=120,
                cwd=str(project_dir),
            )
            passed = result.returncode == 0
            return {
                "passed": passed,
                "output": result.stdout,
                "error": result.stderr if not passed else "",
            }
        except subprocess.TimeoutExpired:
            return {"passed": False, "error": "Tests timed out (120s)"}
        except Exception as e:
            return {"passed": False, "error": str(e)}
