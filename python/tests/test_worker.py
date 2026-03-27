"""Tests for the Python worker."""

import json
import pytest
from unittest.mock import AsyncMock, patch, MagicMock


def test_worker_import():
    """Smoke test: worker module can be imported."""
    import sys
    from pathlib import Path
    sys.path.insert(0, str(Path(__file__).parent.parent / "src"))
    from worker import process_request


@pytest.mark.asyncio
async def test_process_idea():
    """Test idea processing flow."""
    import sys
    from pathlib import Path
    sys.path.insert(0, str(Path(__file__).parent.parent / "src"))
    from worker import process_request
    from llm_client import LLMClient

    mock_llm = MagicMock(spec=LLMClient)
    mock_llm.chat_json = AsyncMock(return_value={
        "project_name": "test-tool",
        "summary": "A test tool",
        "tool_type": "simple-script",
        "has_external_deps": False,
        "tech_stack": ["python"],
        "steps": ["Step 1"],
        "approved": True,
        "score": 8,
        "issues": [],
        "suggestions": [],
        "security_concerns": [],
    })

    with patch("agents.planner.PlannerAgent.__init__", return_value=None), \
         patch("agents.planner.PlannerAgent.analyze", new_callable=AsyncMock, return_value={
             "project_name": "test-tool",
             "summary": "A test tool",
         }), \
         patch("agents.critic.CriticAgent.__init__", return_value=None), \
         patch("agents.critic.CriticAgent.review", new_callable=AsyncMock, return_value={
             "approved": True,
         }):
        result = await process_request("process_idea", {"idea": "Build a test"}, mock_llm)
        assert "project_name" in result


@pytest.mark.asyncio
async def test_research():
    """Test research flow."""
    import sys
    from pathlib import Path
    sys.path.insert(0, str(Path(__file__).parent.parent / "src"))
    from worker import process_request
    from llm_client import LLMClient

    mock_llm = MagicMock(spec=LLMClient)
    mock_llm.chat = AsyncMock(return_value="Research results here")

    with patch("agents.researcher.ResearcherAgent.__init__", return_value=None), \
         patch("agents.researcher.ResearcherAgent.research", new_callable=AsyncMock, return_value="Research results"):
        result = await process_request("research", {"query": "test query"}, mock_llm)
        assert "answer" in result
