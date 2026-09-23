# 🥔 Potato CLI

**Autonomous Super Loop Agent Engine** — a native Rust CLI that drives an LLM through a fully autonomous ReAct loop to architect, scaffold, implement, and verify software projects on your local filesystem.

## Quick Start

```bash
# Via npx (no install required)
npx potato-cli "Build a REST API in Go with SQLite"

# Or install globally
npm install -g potato-cli
potato "Build a rate limiter microservice in Rust"
```

## Requirements

- **Node.js ≥ 18** (for npx/npm distribution)
- **An LLM API key** — set one of:
  - `OPENAI_API_KEY` (default, uses `https://api.openai.com/v1`)
  - `POTATO_API_KEY` + `POTATO_API_BASE` (any OpenAI-compatible endpoint)

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `OPENAI_API_KEY` | — | API key for OpenAI or compatible provider |
| `POTATO_API_KEY` | — | Alternative API key env var |
| `OPENAI_BASE_URL` | `https://api.openai.com/v1` | Base URL for the LLM API |
| `POTATO_API_BASE` | — | Alternative base URL env var |
| `POTATO_MODEL` | `gpt-4o` | Model identifier |
| `POTATO_MAX_TURNS` | `200` | Maximum ReAct turns before abort |

## Usage

```bash
# Basic usage
potato "Build a CLI calculator in Python with pytest"

# With verbose logging
potato -vv "Create an Express.js REST API with JWT auth"

# Custom turn limit
potato --max-turns 50 "Add rate limiting to the existing server"
```

## How It Works

Potato runs a **Super Loop** that autonomously:

1. **SPECIFICATION** — Analyzes your objective, writes `SPEC.md` and `ROADMAP.json`
2. **BOOTSTRAP** — Initializes the project environment and dependencies
3. **IMPLEMENTATION** — Writes code file-by-file in dependency order
4. **VERIFY & REPAIR** — Runs the build/test suite, surgically patches failures

### Anti-Oscillation Guards

- **Surgical patching** — never regenerates entire files for small fixes
- **2-Strike Strategy Shift** — pivots approach after 2 identical failures
- **Git Checkpointing** — auto-commits on green, rolls back after 3 failed cycles
- **Context Window Management** — trims conversation history to stay within limits

## Architecture

```
potato-cli/
├── crates/potato-cli/src/
│   ├── main.rs              # CLI entrypoint (clap)
│   ├── lib.rs               # Library root
│   ├── types/               # Action, Phase, ExecutionResult schemas
│   ├── llm/                 # LLM client (retry, logging) + system prompt
│   ├── engine/              # Super loop orchestrator + action dispatcher
│   └── tools/               # Filesystem, shell exec, git checkpoint tools
├── bin/run.js               # NPM binary shim (zero-dep Node launcher)
├── npm/                     # Platform-specific binary packages
│   ├── darwin-arm64/
│   ├── darwin-x64/
│   ├── linux-x64/
│   ├── linux-arm64/
│   └── win32-x64/
└── .github/workflows/       # CI: cross-compile + NPM publish
```

## Building from Source

```bash
# Requires Rust toolchain
cargo build --release -p potato-cli

# Binary is at target/release/potato (or potato.exe on Windows)
```

## License

MIT
