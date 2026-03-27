# Codex AI

**Automated Tool Builder** — Turn ideas into 24/7 running tools via Telegram.

Send an idea to your Telegram group → AI agents plan, code, test, and deploy it as a Docker container running on your VPS. All managed through Telegram Forum Topics.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                 Telegram Group                    │
│  ┌──────┐ ┌────────┐ ┌─────────┐ ┌───────────┐ │
│  │Ideas │ │Research│ │Dashboard│ │Tool Mgmt  │ │
│  └──┬───┘ └───┬────┘ └────┬────┘ └─────┬─────┘ │
└─────┼─────────┼───────────┼─────────────┼───────┘
      │         │           │             │
      ▼         ▼           ▼             ▼
┌─────────────────────────────────────────────────┐
│              Rust Core (~15MB RAM)               │
│  ┌─────────┐ ┌──────────┐ ┌──────────────────┐ │
│  │ Gateway │ │Orchestr. │ │   Supervisor     │ │
│  │ (Bot +  │ │(Queue +  │ │  (Process Mgr +  │ │
│  │ Router) │ │ Builder) │ │   Scheduler)     │ │
│  └────┬────┘ └─────┬────┘ └──────────────────┘ │
│       │            │                             │
│       ▼            ▼                             │
│  ┌─────────────────────┐                        │
│  │  Python Bridge      │ JSON/stdin/stdout      │
│  └──────────┬──────────┘                        │
└─────────────┼───────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────┐
│           Python Workers (~50MB per call)        │
│  ┌────────┐ ┌──────┐ ┌─────┐ ┌──────┐         │
│  │Planner │ │Critic│ │Coder│ │Tester│ ...      │
│  └────────┘ └──────┘ └─────┘ └──────┘         │
│              LLM API (OpenRouter/OpenAI)         │
└─────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────┐
│           Deployed Tools (Docker)                │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐          │
│  │ Tool A  │ │ Tool B  │ │ Tool C  │ ...      │
│  │ 128-512M│ │ 128-512M│ │ 128-512M│          │
│  └─────────┘ └─────────┘ └─────────┘          │
└─────────────────────────────────────────────────┘
```

## Hybrid Rust + Python

| Component | Language | Why |
|-----------|----------|-----|
| Telegram Bot | Rust (teloxide) | Low RAM, fast, concurrent |
| Message Router | Rust | Pattern matching, zero allocation |
| Build Queue | Rust (tokio) | Semaphores, async channels |
| RAM Guard | Rust (sysinfo) | Real-time memory monitoring |
| Process Supervisor | Rust | Docker management, health checks |
| Scheduler | Rust (cron) | Lightweight cron jobs |
| **LLM Agents** | **Python** | Rich ecosystem, httpx, rapid iteration |
| **Memory** | **Python** | JSON-based project memory |

**Communication**: Rust spawns Python workers as subprocesses. JSON protocol over stdin/stdout.

## Telegram Forum Topics

| Topic | Purpose |
|-------|---------|
| 💡 Ideas | Send tool ideas here |
| 🔍 Research | Ask research questions |
| 📊 Dashboard | System status and metrics |
| 🛠 Tool Management | /list, /stop, /restart, /logs |
| 🤖 Agent Logs | Build pipeline progress |
| 🔧 tool-{name} | Auto-created per deployed tool |

## Quick Start

### Prerequisites
- Rust 1.75+
- Python 3.11+
- Docker (for deploying tools)
- Telegram Bot Token ([BotFather](https://t.me/BotFather))
- LLM API Key (OpenRouter, OpenAI, etc.)

### Setup

```bash
# Clone
git clone https://github.com/volatoken/codex-ai.git
cd codex-ai

# Setup (Linux/Mac)
chmod +x scripts/setup.sh
./scripts/setup.sh

# Setup (Windows)
powershell -ExecutionPolicy Bypass -File scripts/setup.ps1
```

### Configure

Edit `.env`:
```env
TELEGRAM_BOT_TOKEN=your_bot_token
TELEGRAM_GROUP_ID=-100xxxxxxxxxx
TELEGRAM_ADMIN_USER_ID=your_user_id
LLM_API_KEY=your_api_key
```

### Run

```bash
make run
```

## Build Pipeline

```
Idea → Planner → Critic → [Approve] → Coder → Tester → Docker Build → Deploy
                    ↑                     ↑
                    └─── Reject ──────────┘ (retry with fixes)
```

**Concurrency limits** (8GB RAM VPS):
- Planning: 3 concurrent
- Docker build: 1 at a time (RAM gated)
- Running tools: 5-8 concurrent

## Project Structure

```
codex-ai/
├── rust/                    # Rust core
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs          # Entry point
│       ├── config.rs        # Settings from .env
│       ├── gateway/         # Telegram bot + router + topics
│       ├── orchestrator/    # Build queue + RAM guard + deployer
│       ├── bridge/          # Python subprocess bridge
│       ├── scheduler/       # Cron jobs
│       └── supervisor/      # Process management
├── python/                  # Python workers
│   ├── requirements.txt
│   ├── src/
│   │   ├── worker.py        # Stdin/stdout JSON worker
│   │   ├── llm_client.py    # LLM API client
│   │   ├── memory.py        # Project memory
│   │   └── agents/          # Planner, Critic, Coder, Tester, DevOps, Researcher
│   ├── prompts/             # Agent system prompts
│   └── tests/
├── docker/base-images/      # Pre-built Docker bases
├── scripts/                 # Setup scripts
├── data/                    # Runtime data (topics, memory)
├── workspace/projects/      # Generated tool projects
├── logs/                    # Log files
├── .env.example
├── .gitignore
├── Makefile
└── README.md
```

## License

MIT
