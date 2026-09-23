# 🥔 Potato CLI

**Autonomous Super Loop Agent Engine** — a native Rust CLI that drives an LLM through a fully autonomous ReAct loop to architect, scaffold, implement, and verify software projects on your local filesystem.

Featuring **20 built-in specialist & meta subagents**, soft-jailed sandboxing, token cost guards, lifecycle hooks, and cross-platform native distribution across **npm, bun, yarn, pnpm, and Docker**.

---

## ⚡ Quick Start

### Via Package Managers (Zero-Install Runner)

```bash
# npm / npx
npx potato-cli "Build a REST API in Go with SQLite"

# Bun / bunx
bunx potato-cli "Build an Axum microservice in Rust"

# pnpm dlx
pnpm dlx potato-cli "Build a fullstack Next.js dashboard"

# Yarn dlx
yarn dlx potato-cli "Create an Express.js server with JWT auth"
```

### Global Installation

```bash
npm install -g potato-cli
# or: bun add -g potato-cli
# or: pnpm add -g potato-cli
# or: yarn global add potato-cli

potato "Build a rate limiter microservice in Rust"
```

### Via Docker

```bash
# Pull or build the image
docker build -t potato-cli .

# Run inside an isolated container with volume mount
docker run --rm -it \
  -e OPENAI_API_KEY="sk-..." \
  -v $(pwd)/workspace:/workspace \
  potato-cli "Build an authentication service in Python"
```

---

## 🤖 Built-in Subagents (20)

Run `potato --list-agents` to view all active agents.

### Core Specialists
| Agent | Category | Role |
|---|---|---|
| **Architect** | `architecture` | Generates `SPEC.md`, system topology, and `ROADMAP.json` |
| **Implementer** | `implementation` | Writes idiomatic, production-grade code module-by-module |
| **Reviewer** | `review` | Performs AST & architectural code reviews |
| **TestWriter** | `testing` | Writes comprehensive unit, integration, and E2E test suites |
| **Repair** | `repair` | Diagnoses compiler errors/test failures and executes surgical patches |
| **SecurityAuditor** | `security` | Audits code for OWASP Top 10 vulnerabilities, injection, and auth flaws |
| **DocGenerator** | `documentation` | Generates README, API specs, and inline code documentation |

### Extended Specialists
| Agent | Category | Role |
|---|---|---|
| **Refactorer** | `refactoring` | Eliminates code smells, reduces cyclomatic complexity, enforces DRY |
| **PerformanceProfiler** | `performance` | Benchmarks hot paths, detects memory allocations, suggests optimizations |
| **Migration** | `migration` | Ports legacy systems across languages/frameworks |
| **DependencyAuditor** | `dependencies` | Audits manifests for known CVEs, license conflicts, and bloat |
| **DevOps** | `devops` | Scaffolds Dockerfiles, GitHub Actions CI/CD workflows, Terraform |
| **Database** | `database` | Generates SQL schemas, migrations, seeders, and query indexes |
| **APIDesigner** | `api_design` | Designs OpenAPI/Swagger contracts and route architectures |

### Meta Agents
| Agent | Category | Role |
|---|---|---|
| **Planner** | `planning` | Decomposes complex multi-system objectives into task DAGs |
| **Retrospective** | `retrospective` | Analyzes execution sessions to output lessons and performance reports |
| **PromptOptimizer** | `prompt_optimization` | Refines and tunes agent instructions based on outcome success |
| **CostOptimizer** | `cost_optimization` | Minimizes token usage and analyzes cost efficiency across turns |
| **Orchestrator** | `orchestration` | Dynamically routes tasks through agent graphs |

---

## 🛠️ CLI Options

```
Usage: potato [OPTIONS] [OBJECTIVE]

Arguments:
  [OBJECTIVE]  Natural language objective (e.g. "Build a CLI in Rust")

Options:
      --max-turns <MAX_TURNS>  Max ReAct turns before abort [default: 200]
      --budget <BUDGET>        Max cost budget in USD (e.g. --budget 5.0)
      --list-agents            List all built-in and custom TOML subagents
      --resume <RESUME>        Resume a checkpointed session by ID
  -v, --verbose...             Verbosity level (-v: info, -vv: debug, -vvv: trace)
  -h, --help                   Print help information
  -V, --version                Print version
```

---

## ⚙️ Configuration (`potato.toml`)

Potato automatically loads configuration layered from `./potato.toml`, `~/.config/potato/config.toml`, and environment variables:

```toml
# potato.toml

[llm]
api_key = "sk-..."
api_base = "https://api.openai.com/v1"
model = "gpt-4o"
temperature = 0.1
timeout_seconds = 180

[cost]
max_cost_usd = 5.00
warn_at_usd = 3.00

[tools]
blocked_commands = ["rm -rf /", "format C:", "mkfs"]

[tools.Reviewer]
allowed_actions = ["read_file", "list_dir", "finish"]

[hooks]
post_write = ".potato/hooks/lint.sh"
post_commit = ".potato/hooks/notify.sh"
on_complete = ".potato/hooks/report.sh"

[[agents]]
name = "StyleEnforcer"
category = "review"
max_turns = 10
model = "gpt-4o-mini"
system_prompt = "Verify all functions have documentation comments."
```

---

## 🛡️ Anti-Oscillation & Safety Guards

- **Unified Diff Display** — Displays instant colorized diffs on every file write or patch
- **Path-Prefix Sandbox** — Soft jail preventing directory traversal (`../`) outside workspace
- **Cost Guard & Token Budget** — Live tracking with `--budget` hard stops
- **Graceful Shutdown** — Two-phase Ctrl+C signal handling auto-commits git checkpoints
- **Anti-Oscillation Detection** — Detects repeated action failures and injects rollback hints

---

## 📦 Architecture

```
potato-cli/
├── crates/potato-cli/src/
│   ├── main.rs              # CLI entrypoint (clap)
│   ├── lib.rs               # Library root
│   ├── types/               # Action, Phase, ExecutionResult schemas
│   ├── llm/                 # Client (retry, token tracking) + prompt builders
│   ├── agents/              # 20 Specialist & Meta agents + TOML CustomAgent
│   ├── engine/              # LoopRunner, CostTracker, ToolPolicy, Hooks, Sandbox
│   └── tools/               # fs, exec, git, diff_display
├── bin/run.js               # Multi-runtime JS shim (npm/bun/yarn/pnpm)
├── Dockerfile               # Multi-stage production container
├── npm/                     # Platform-specific packages (darwin, linux, win32)
└── .github/workflows/       # Automated CI & cross-compilation release pipeline
```

---

## 📄 License

MIT
