# 🥔 Potato CLI — Project Architecture & Context

**Potato CLI** is a high-performance native Rust autonomous "Super Loop" coding agent engine. It drives an LLM through autonomous ReAct cycles to architect, scaffold, implement, and verify software projects on local filesystems with zero monoliths, strict sandboxing, and token budget guards.

---

## 🏗️ Core Architecture

- **Crate Root**: `crates/potato-cli`
  - `src/main.rs`: CLI entrypoint (Clap), subcommand dispatch, pre-flight checks, interactive REPL bridge.
  - `src/lib.rs`: Core engine library exports.
  - `src/agents/`: 20 Specialist & Meta subagents orchestrated by a central [Coordinator](file:///c:/Users/Lenovo/potato-cli/crates/potato-cli/src/agents/coordinator.rs).
  - `src/engine/`:
    - `arcade.rs`: 8-bit arcade marquee, bootup sequence animation, sprite frame sets, stage headers, and victory/game over fanfare.
    - `spinner.rs`: 8-bit animated loaders with neon CRT color cycling and Invader / Pacman themes.
    - `loop_runner.rs`: Super Loop execution engine with anti-oscillation rollback guards.
    - `repl.rs`: Interactive terminal REPL session (similar to Claude Code and Antigravity) with slash commands and onboarding wizard.
    - `context/`: `ContextAssembler` (system/toolchain probe) & `ContextCompressor` (dense turn condensation).
    - `cost_tracker.rs`: Token usage tracking and hard USD budget guards.
    - `tool_policy.rs`: Global blocked commands and per-agent tool allowlists.
    - `hooks.rs`: Lifecycle hooks (`pre_write`, `post_write`, `pre_exec`, `post_commit`, `on_complete`, `on_failure`).
    - `sandbox.rs`: Soft-jail path traversal prevention.
    - `session.rs`: Checkpoint state serialization and resumption.
  - `src/tools/`:
    - `fs_tools.rs`: Sandboxed file read, write, and line-range inspection.
    - `exec_tools.rs`: Command execution with configurable timeouts.
    - `git_tools.rs`: Git commit, diff extraction, and rollback checkpoints.
    - `diff_display.rs`: Colorized unified LCS diff renderer.
    - `ast_validator.rs`: Pre-write syntax gatekeeper (Rust, JSON, TOML, balanced delimiters).
  - `src/orchestration/`:
    - `dag.rs`: Directed Acyclic Graph wave scheduler with cycle detection.
    - `consensus.rs`: Multi-critic scoring and weighted voting engine.
    - `speculative.rs`: Branch-and-prune speculative execution via ephemeral Git branches.
    - `state.rs`: Pure deterministic reducer state tracking.

---

## 🤖 Built-in Specialist & Meta Subagents (20)

| Subagent | Category | Role |
|---|---|---|
| **Architect** | `architecture` | Produces `SPEC.md`, system topology, and `ROADMAP.json` |
| **Implementer** | `implementation` | Scaffolds and writes idiomatic source code |
| **Reviewer** | `review` | Performs code review, static analysis, and security sanity checks |
| **TestWriter** | `testing` | Generates comprehensive unit and integration test suites |
| **Repair** | `repair` | Surgical fault localization and compiler error patching |
| **SecurityAuditor** | `security` | Audits dependencies, CVEs, secret leaks, and insecure permissions |
| **DocGenerator** | `documentation` | Generates README, API specs, and inline code comments |
| **Refactorer** | `refactoring` | Restructures code for modularity, clean architecture, and SoC |
| **PerformanceProfiler** | `performance` | Analyzes bottlenecks, algorithmic complexity, and memory footprints |
| **Migration** | `migration` | Handles database schema migrations and framework upgrades |
| **DependencyAuditor** | `dependencies` | Audits outdated dependencies and package compatibility |
| **DevOps** | `devops` | Generates Dockerfiles, CI/CD pipelines, and cloud deployment configs |
| **Database** | `database` | Schema modeling, index tuning, and ORM query optimization |
| **APIDesigner** | `api_design` | REST, GraphQL, and gRPC interface specifications |
| **Planner** | `planning` | Decomposes complex multi-system objectives into task DAGs |
| **Retrospective** | `retrospective` | Analyzes execution sessions to extract lessons and efficiency reports |
| **PromptOptimizer** | `prompt_optimization` | Refines and tunes agent instructions based on outcome success |
| **CostOptimizer** | `cost_optimization` | Minimizes token usage and analyzes cost efficiency across turns |
| **Orchestrator** | `orchestration` | Dynamically routes tasks through agent graphs |
| **CustomAgent** | Dynamic | Loaded dynamically from TOML configurations (`potato.toml`) |

---

## 🚀 Execution & Command Surface

- **Global Runners & Aliases**:
  - `pot` or `potato`: Invoking without arguments starts the **Interactive REPL**.
  - `pot "<objective>"`: Executes autonomous Super Loop for the given objective.
- **Interactive REPL Commands**:
  - `/help`: Displays command cheat sheet.
  - `/init`: Scaffolds `.potato/` workspace and starter `potato.toml`.
  - `/audit`: Runs security and dependency vulnerability audit.
  - `/review`: Reviews uncommitted Git working tree diffs (pre-flight checks clean status).
  - `/spec <goal>`: Generates architectural specification and roadmap without modifying source.
  - `/pipeline <goal>`: Runs full 5-stage pipeline (`Architect` → `Implementer` → `Reviewer` → `Repair` → `Security`).
  - `/agents`: Lists all 20 registered specialist subagents and dynamic TOML agents.
  - `/model [name]`: Views or switches the active LLM model.
  - `/budget [usd]`: Views or updates the session budget.
  - `/cost`: Displays real-time token counts and USD expenditure.
  - `/session`: Inspects active session memory, context turns, and mutated files.
  - `/sessions`: Lists all historical saved sessions in `.potato/sessions/`.
  - `/resume [id]`: Resumes a previous session checkpoint (latest by default, or by session ID).
  - `/reset`: Resets session memory and starts a clean session (or `/new`).
  - `/brain`: Inspects project Brain knowledge, architectural conventions, and learned lessons.
  - `/cache [clear]`: Views LLM response cache performance metrics or clears disk cache.
  - `/clear`: Clears the terminal screen.
  - `/exit` or `/quit`: Cleanly terminates the interactive session.
- **CLI Subcommands & Options**:
  - `potato init`
  - `potato audit`
  - `potato review`
  - `potato spec "<goal>"`
  - `potato pipeline "<goal>"`
  - `potato --resume [id]` (or `pot --resume`): Resumes a previous session from disk.

---

## 📦 Distribution & Multi-Runtime Setup

1. **Native Windows / Terminals**:
   - `install.cmd`: Zero-dependency installer deploying `potato.exe` and `pot.exe` to `%USERPROFILE%\.cargo\bin\` and `%APPDATA%\npm\`.
   - Bypasses PowerShell's `ExecutionPolicy Restricted` by using native binaries over unsigned scripts.
   - Project-local wrappers: `pot.cmd`, `potato.cmd`, `pot.ps1`, `potato.ps1`, `pot`, `potato`.
2. **Package Managers**:
   - `package.json` exposes `potato`, `potato-cli`, and `pot` bin entries.
   - `bin/run.js` provides multi-strategy native binary resolution across npm, Bun, Yarn, and pnpm.
3. **Docker & Containers**:
   - `Dockerfile`: Multi-stage build (`rust:bookworm` builder + `debian:bookworm-slim` runtime).
   - Bundles `Node.js 20`, `Bun 1.4`, `Yarn`, `pnpm`, `Python 3.11`, `Git`, and `potato`.
   - `docker-compose.yml`: Mounts host workspace to `/workspace` and passes API keys.

---

## 🛡️ Quality & Verification

- **Tests**: 21/21 passing unit and integration tests (`cargo test`).
- **Linter**: Zero warnings on `cargo clippy --all-targets`.
- **Pre-Write Gate**: In-memory AST checks run in ~35 µs before writing files to disk.
