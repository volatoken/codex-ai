"""Tests for agent modules."""

import pytest
from unittest.mock import AsyncMock, MagicMock
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "src"))


@pytest.mark.asyncio
async def test_planner_analyze():
    from agents.planner import PlannerAgent
    from llm_client import LLMClient

    mock_llm = MagicMock(spec=LLMClient)
    mock_llm.chat_json = AsyncMock(return_value={
        "project_name": "price-tracker",
        "summary": "Tracks prices",
        "tool_type": "cron-job",
    })

    agent = PlannerAgent(mock_llm)
    result = await agent.analyze("Build a price tracker")
    assert result["project_name"] == "price-tracker"
    mock_llm.chat_json.assert_awaited_once()


@pytest.mark.asyncio
async def test_critic_review():
    from agents.critic import CriticAgent
    from llm_client import LLMClient

    mock_llm = MagicMock(spec=LLMClient)
    mock_llm.chat_json = AsyncMock(return_value={
        "approved": True,
        "score": 8,
        "issues": [],
    })

    agent = CriticAgent(mock_llm)
    result = await agent.review({"project_name": "test"})
    assert result["approved"] is True


@pytest.mark.asyncio
async def test_coder_generate():
    from agents.coder import CoderAgent
    from llm_client import LLMClient

    mock_llm = MagicMock(spec=LLMClient)
    mock_llm.chat_json = AsyncMock(return_value={
        "main.py": "print('hello')",
        "requirements.txt": "",
    })

    agent = CoderAgent(mock_llm)
    result = await agent.generate("test-project", {"files": {"main.py": "entry"}})
    assert "main.py" in result


@pytest.mark.asyncio
async def test_researcher_research():
    from agents.researcher import ResearcherAgent
    from llm_client import LLMClient

    mock_llm = MagicMock(spec=LLMClient)
    mock_llm.chat = AsyncMock(return_value="Research results about topic")

    agent = ResearcherAgent(mock_llm)
    result = await agent.research("What is Docker?")
    assert "Research results" in result
