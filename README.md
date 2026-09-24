# 🥔 Potato CLI

```
  ╔══════════════════════════════════════════════════════════════╗
  ║    👾 🕹️  P O T A T O   C L I  🕹️ 👾                       ║
  ║       ★ 8-BIT AUTONOMOUS SUPER LOOP AGENT ENGINE ★          ║
  ║       INSERT COIN • 1-UP READY • TYPE /help FOR CMDS         ║
  ╚══════════════════════════════════════════════════════════════╝
```

[![Tests](https://img.shields.io/badge/tests-21%2F21%20passing-brightgreen.svg)]()
[![Rust](https://img.shields.io/badge/rust-2021%20edition-orange.svg)]()
[![Node](https://img.shields.io/badge/node-%3E%3D18-blue.svg)]()
[![License](https://img.shields.io/badge/license-MIT-purple.svg)]()

**Potato CLI** is a high-performance native Rust autonomous "Super Loop" coding agent engine. It drives an LLM through autonomous ReAct cycles to architect, scaffold, implement, and verify software projects on local filesystems with zero monoliths, strict sandboxing, token budget guards, and retro 8-bit arcade animations.

---

## ⚡ Quick Start

### 1. Instant Run via NPX (No Installation Needed)

```bash
# Launch interactive REPL mode:
npx potato-agent

# Or run natural language objectives directly:
npx potato-agent "Build a REST API in Go with SQLite"
```

### 2. Global Installation

Install globally via your favorite package manager:

```bash
# NPM
npm install -g potato-agent

# Bun
bun add -g potato-agent

# pnpm
pnpm add -g potato-agent

# Yarn
yarn global add potato-agent
```

Once installed, use either `pot` or `potato`:
```bash
pot
```

### 3. Standalone Native Shell Installers (No Node.js or Rust Required)

- **Windows (PowerShell / Command Prompt)**:
  ```cmd
  install.cmd
  ```
  *Deploys native `potato.exe` and `pot.exe` to `%USERPROFILE%\.cargo\bin` and `%APPDATA%\npm`, automatically bypassing ExecutionPolicy restrictions.*

- **macOS & Linux (Bash / Zsh)**:
  ```bash
  curl -fsSL https://raw.githubusercontent.com/user/potato-cli/master/scripts/install.sh | bash
  ```

### 4. Via Cargo (Rust Toolchain)

```bash
cargo install --git https://github.com/user/potato-cli
```

### 5. Via Docker (Completely Sandboxed)

```bash
# Run with volume mount:
docker run -it --rm -v "${PWD}:/workspace" -e OPENAI_API_KEY="sk-..." potatocli/potato

# Or via Docker Compose:
docker compose run --rm potato
```

---

## 🎮 8-Bit Arcade Engine & Visuals

Potato CLI features a full 8-bit retro arcade aesthetic designed for modern developer terminals:

- **Arcade Boot Sequence**: Rapid hardware and neural chip bootup animation on launch.
- **8-Bit Loaders & Color Cycling**: Space Invader (`👾 [ ▰ ▰ ▱ ▱ ]`) and Pac-Man sprite frames with neon CRT color gradients (cyan, yellow, magenta, green).
- **Stage Progression & Score Counters**: Every turn renders as an arcade stage with dynamic score counters derived from tokens and turns.
- **Stage Clear Fanfare**: 8-bit victory banners upon successful objective completion.
- **Game Over & Rollback Banners**: Visual continue counter and automated state rollback on consecutive compiler/action failures.
- **Pixel Progress Meters**: Visual health/mana style meters for token budgets (`[▰▰▰▰▱▱] $0.42 / $5.00`) and session turn memory.

---

## 🛡️ Deadlock-Free & Anti-Stall Execution Engine

Potato CLI is engineered to prevent hangs, deadlocks, and silent timeouts:

1. **Deadlock-Free Pipe Streaming**: Concurrently drains `stdout` and `stderr` in dedicated background threads, eliminating OS pipe buffer deadlocks when commands emit large volumes of output (>64KB).
2. **Process Tree Termination**: On Windows, timeouts trigger `taskkill /F /T /PID` to terminate the entire process hierarchy, preventing orphaned background processes.
3. **LLM Watchdog Timeouts**: 15s connect timeout, 75s request timeout, and 30s body stream guard with exponential backoff on HTTP 429/503.
4. **Live Buffering Badges**: Spinners automatically display runtime elapsed seconds and dynamic buffering status badges (`(16s) [buffering...]` / `(45s) [buffering... still working]`).
5. **Turn-Level Auto-Recovery**: Detects network buffering stalls and performs non-destructive automatic retries before aborting sessions.

---

## 🧠 Session Memory, Cache & Project Brain

- **Persistent Session Memory**: Automatic checkpoints saved to `.potato/sessions/` after every turn. Resume anytime via `/resume [id]` or `potato --resume [id]`.
- **Response Caching**: In-memory + disk cache (`.potato/cache/`) with deterministic SHA hashing of `(model, messages)`. Instant 0-token replay for identical turns.
- **Project Brain**: Self-updating cognitive repository in `.potato/brain/` tracking `KNOWLEDGE.md`, `LESSONS.md`, `MUTATED_FILES.json`, and architectural conventions.

---

## 💬 Interactive REPL & Slash Commands

Type `pot` without arguments to launch the interactive terminal REPL:

```
👾 POTATO ❯ /help
```

| Command | Description |
|---|---|
| `/help` | Displays command cheat sheet |
| `/init` | Scaffolds `.potato/` workspace and starter `potato.toml` |
| `/audit` | Runs comprehensive security and dependency vulnerability audit |
| `/review` | Reviews uncommitted Git working tree diffs |
| `/spec <goal>` | Generates architectural specification and roadmap without modifying source |
| `/pipeline <goal>` | Runs 5-stage pipeline (`Architect` → `Implementer` → `Reviewer` → `Repair` → `Security`) |
| `/agents` | Lists all 20 registered specialist subagents and custom TOML agents |
| `/model [name]` | Views or switches active LLM model (e.g. `/model o3-mini` or `/model 1`) |
| `/budget [usd]` | Views or updates session USD budget |
| `/cost` | Displays real-time token usage and cost metrics |
| `/session` | Inspects active session memory, context turns, and mutated files |
| `/sessions` | Lists all historical saved sessions in `.potato/sessions/` |
| `/resume [id]` | Resumes a previous session checkpoint (latest by default, or by session ID) |
| `/reset` | Resets session memory and starts a clean session (or `/new`) |
| `/brain` | Inspects project Brain knowledge, architectural conventions, and learned lessons |
| `/cache [clear]` | Views response cache hit rates or clears disk cache |
| `/clear` | Clears terminal screen |
| `/exit` | Cleanly terminates the interactive session |

---

## 🤖 Built-in Subagents (20)

Run `pot /agents` or `potato --list-agents` to inspect registered agents:

### Core Specialists
| Agent | Category | Role |
|---|---|---|
| **Architect** | `architecture` | Produces `SPEC.md`, system topology, and `ROADMAP.json` |
| **Implementer** | `implementation` | Scaffolds and writes idiomatic source code module-by-module |
| **Reviewer** | `review` | Performs AST, style, and architectural reviews |
| **TestWriter** | `testing` | Writes comprehensive unit, integration, and E2E test suites |
| **Repair** | `repair` | Surgical fault localization and compiler error patching |
| **SecurityAuditor** | `security` | Audits for vulnerabilities, CVEs, secret leaks, and insecure permissions |
| **DocGenerator** | `documentation` | Generates README, API specs, and inline code documentation |

### Extended Specialists
| Agent | Category | Role |
|---|---|---|
| **Refactorer** | `refactoring` | Restructures code for modularity, clean architecture, and SoC |
| **PerformanceProfiler** | `performance` | Analyzes bottlenecks, algorithmic complexity, and memory footprints |
| **Migration** | `migration` | Handles database schema migrations and framework upgrades |
| **DependencyAuditor** | `dependencies` | Audits outdated dependencies and package compatibility |
| **DevOps** | `devops` | Generates Dockerfiles, CI/CD pipelines, and cloud deployment configs |
| **Database** | `database` | Schema modeling, index tuning, and ORM query optimization |
| **APIDesigner** | `api_design` | REST, GraphQL, and gRPC interface specifications |

### Meta Agents
| Agent | Category | Role |
|---|---|---|
| **Planner** | `planning` | Decomposes complex multi-system objectives into task DAGs |
| **Retrospective** | `retrospective` | Analyzes execution sessions to extract lessons and efficiency reports |
| **PromptOptimizer** | `prompt_optimization` | Refines and tunes agent instructions based on outcome success |
| **CostOptimizer** | `cost_optimization` | Minimizes token usage and analyzes cost efficiency across turns |
| **Orchestrator** | `orchestration` | Dynamically routes tasks through agent graphs |
| **CustomAgent** | Dynamic | Loaded dynamically from TOML configurations (`potato.toml`) |

---

## ⚙️ Configuration (`potato.toml`)

Potato automatically loads layered configurations from `./potato.toml`, `~/.config/potato/config.toml`, and environment variables:

```toml
# potato.toml

[llm]
api_key = "sk-..."
api_base = "https://api.openai.com/v1"
model = "gpt-4o"
temperature = 0.1

[cost]
max_cost_usd = 5.00
warn_at_usd = 3.00

[tools]
blocked_commands = ["rm -rf /", "format C:", "mkfs"]

[tools.Reviewer]
allowed_actions = ["read_file", "list_dir", "finish"]

[hooks]
pre_write = ".potato/hooks/check.sh"
post_write = ".potato/hooks/lint.sh"
on_complete = ".potato/hooks/notify.sh"

[[agents]]
name = "StyleEnforcer"
category = "review"
max_turns = 10
model = "gpt-4o-mini"
system_prompt = "Verify all functions have documentation comments."
```

---

## 🏗️ Repository Architecture

```tree
potato-cli/
├── crates/potato-cli/src/
│   ├── main.rs              # CLI entrypoint (Clap), pre-flight checks, REPL dispatch
│   ├── lib.rs               # Engine library exports
│   ├── agents/              # 20 Specialist & Meta subagents + Coordinator
│   ├── engine/
│   │   ├── arcade.rs        # 8-bit marquee, boot animation, frames & fanfare
│   │   ├── spinner.rs       # 8-bit animated loaders with neon CRT color cycling
│   │   ├── loop_runner.rs   # Super Loop engine with deadlock & stall recovery
│   │   ├── repl.rs          # Interactive terminal REPL session & onboarding wizard
│   │   ├── session.rs       # Checkpoint state serialization & resumption
│   │   ├── brain.rs         # Project Brain knowledge & lesson repository
│   │   ├── cache.rs         # In-memory & disk LLM response cache
│   │   ├── cost_tracker.rs  # Token usage tracking & hard USD budget guards
│   │   └── sandbox.rs       # Soft-jail path traversal prevention
│   └── tools/               # fs, exec (deadlock-free), git, ast_validator, diff
├── bin/run.js               # Multi-runtime JS launcher (npm/bun/yarn/pnpm)
├── scripts/postinstall.js   # Automated Windows .ps1 cleanup hook for npm
├── npm/                     # Platform-specific native binary packages
└── .github/workflows/       # Matrix cross-compilation & npm deployment pipeline
```

---

## 📄 License

MIT © 2026 Potato CLI Contributors.
