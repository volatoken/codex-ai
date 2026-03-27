# Codex AI

**Automated Tool Builder** — Turn ideas into 24/7 running tools via Telegram.

Send an idea to your Telegram group → DeerFlow AI agents plan, code, test, and deploy it as a Docker container running on your VPS. All managed through Telegram Forum Topics.

## Architecture

```
┌─────────────────────────────────────────────────┐
│            Telegram Supergroup (Forum)            │
│  ┌──────┐ ┌────────┐ ┌─────────┐ ┌───────────┐ │
│  │Ideas │ │Research│ │Dashboard│ │Tool Mgmt  │ │
│  └──┬───┘ └───┬────┘ └────┬────┘ └─────┬─────┘ │
└─────┼─────────┼───────────┼─────────────┼───────┘
      │         │           │             │
      ▼         ▼           ▼             ▼
┌─────────────────────────────────────────────────┐
│           Rust Gateway (~15MB RAM)               │
│  ┌─────────┐ ┌──────────┐ ┌──────────────────┐ │
│  │ Gateway │ │Orchestr. │ │   Supervisor     │ │
│  │ (Bot +  │ │(Queue +  │ │  (Process Mgr +  │ │
│  │ Router +│ │ Builder +│ │   Scheduler)     │ │
│  │ Topics) │ │RAM Guard)│ │                  │ │
│  └────┬────┘ └─────┬────┘ └──────────────────┘ │
│       │            │                             │
│       ▼            ▼                             │
│  ┌─────────────────────┐                        │
│  │  DeerFlow Bridge    │  HTTP / SSE Streaming  │
│  └──────────┬──────────┘                        │
└─────────────┼───────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────┐
│        DeerFlow 2.0 Backend (~300MB RAM)         │
│  ┌──────────────────────────────────────┐       │
│  │        LangGraph Agent Server         │       │
│  │  ┌────────┐ ┌──────┐ ┌───────────┐  │       │
│  │  │Planner │ │Coder │ │Researcher │  │       │
│  │  └────────┘ └──────┘ └───────────┘  │       │
│  │  ┌──────────┐ ┌────────┐ ┌───────┐  │       │
│  │  │ Sandbox  │ │ Memory │ │Skills │  │       │
│  │  │ (Docker) │ │ (Mem0) │ │ (MD)  │  │       │
│  │  └──────────┘ └────────┘ └───────┘  │       │
│  └──────────────────────────────────────┘       │
│              LLM API (OpenRouter/CliproxyAPI)     │
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

## Rust Gateway + DeerFlow Backend

| Component | Language | Why |
|-----------|----------|-----|
| Telegram Bot | Rust (teloxide) | Low RAM, fast, Forum Topics support |
| Message Router | Rust | Pattern matching, zero allocation |
| Build Queue | Rust (tokio) | Semaphores, async channels |
| RAM Guard | Rust (sysinfo) | Real-time memory monitoring |
| Process Supervisor | Rust | Docker management, health checks |
| Scheduler | Rust (cron) | Lightweight cron jobs |
| **AI Agents** | **DeerFlow (LangGraph)** | Sub-agents, sandbox, self-fix loops, memory |
| **Code Execution** | **DeerFlow Sandbox** | Docker-isolated code execution |
| **Research** | **DeerFlow** | Web search, crawling, analysis |
| **Memory** | **DeerFlow (Mem0)** | Long-term memory across conversations |

**Communication**: Rust Gateway calls DeerFlow via HTTP API (REST + SSE streaming). DeerFlow handles all AI/LLM work internally.

### Why Both Rust + DeerFlow?

- **DeerFlow** doesn't support Telegram **Forum Topics** (only basic reply threading)
- Rust Gateway adds: Forum Topics routing, Build Queue, RAM Guard, Process Supervisor
- DeerFlow adds: LangGraph multi-agent orchestration, sandbox code execution, self-fix loops, progressive skills
- Idle RAM: Rust ~15MB + DeerFlow ~300MB = **~315MB** (leaves ~7.7GB on 8GB VPS)

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
- Docker & Docker Compose
- Telegram Bot Token ([BotFather](https://t.me/BotFather))
- LLM API Key (OpenRouter, CliproxyAPI, etc.)

### Setup

```bash
# Clone
git clone https://github.com/volatoken/codex-ai.git
cd codex-ai

# Configure
cp .env.example .env
# Edit .env with your tokens and API keys
```

### Run (Docker Compose — recommended)

```bash
# Start full stack: Rust Gateway + DeerFlow
make up

# View logs
make logs

# Stop
make down
```

### Run (Manual — development)

```bash
# Terminal 1: Start DeerFlow
docker compose up deerflow -d

# Terminal 2: Build and run Rust gateway
make run
```

## Build Pipeline

```
Idea → DeerFlow Planner → [Approve] → DeerFlow Coder → DeerFlow Review/Test
                                            ↑                    │
                                            └── Self-Fix Loop ──┘
                                                     │
                                              Docker Build → Deploy
```

**DeerFlow advantages over old pipeline**:
- Sub-agents run in parallel with isolated context
- Sandbox executes and tests code in Docker containers
- Self-fix loops: if tests fail, DeerFlow automatically debugs and retries
- Long-term memory remembers past builds and patterns
- Progressive skills loaded based on task context

**Concurrency limits** (8GB RAM VPS):
- DeerFlow planning: 3 concurrent agent threads
- Docker build: 1 at a time (RAM gated by Rust)
- Running tools: 5-8 concurrent

## Project Structure

```
codex-ai/
├── rust/                    # Rust Gateway
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs          # Entry point + DeerFlow health check
│       ├── config.rs        # Settings from .env
│       ├── gateway/         # Telegram bot + router + Forum Topics
│       ├── orchestrator/    # Build queue + RAM guard + deployer
│       ├── bridge/          # DeerFlow HTTP/SSE bridge
│       ├── scheduler/       # Cron jobs (health, RAM reports)
│       └── supervisor/      # Docker process management
├── deerflow/                # DeerFlow configuration
│   ├── config.yaml          # DeerFlow server config
│   └── skills/custom/       # Custom skills for DeerFlow agents
│       ├── tool-builder/    # Skill: build tools from ideas
│       └── tool-supervisor/ # Skill: monitor running tools
├── docker/                  # Docker build files
│   └── Dockerfile.gateway   # Multi-stage Rust build
├── docker-compose.yml       # Full stack orchestration
├── data/                    # Runtime data (topics.json, etc.)
├── workspace/projects/      # Generated tool projects
├── logs/                    # Log files
├── scripts/                 # Setup scripts
├── .env.example             # Environment template
├── .gitignore
├── Makefile
└── README.md
```

## License

MIT
