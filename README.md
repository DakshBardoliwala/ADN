# ADN — Architectural Discovery Navigation

### Your AI finally understands your codebase.

[![Version](https://img.shields.io/badge/version-0.2.0-blue)](https://github.com/DakshBardoliwala/ADN/releases)
[![License](https://img.shields.io/badge/license-Apache--2.0-green)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange)](https://www.rust-lang.org/)
[![MCP Ready](https://img.shields.io/badge/MCP-Claude%20%7C%20Cursor%20%7C%20Codex-purple)](https://modelcontextprotocol.io/)

---

## The Problem with AI Code Search

AI agents read your code like a document — they find the word `authenticate`, but they don't know it's a method on `AuthService`, that three API routes call it, or that renaming it silently breaks your middleware.

**ADN fixes this.** It builds a structured knowledge graph of your codebase and plugs it directly into your AI via MCP. Your agent stops guessing and starts reasoning — about structure, dependencies, and change impact.

---

## What You Get

- **Recursive Code Intelligence** — Understands the full structure of your project: files, classes, methods, and the relationships between them. Not just text, but a live map.

- **Human-Readable Lookup** — `inspect` and `trace` work with `--name` and `--file`, so you can target `AuthService` in `src/auth.py` directly instead of hunting for UUIDs first.

- **Impact Radius Tracing** — Ask *"what breaks if I change this?"* and get a precise, multi-level dependency tree. Trace depth is configurable, so you can choose a quick local check or a broader upstream scan.

- **Global Import Resolution** — Correctly links `from module import ClassName` to the exact definition, even across files that were indexed in any order. No dangling references.

- **Blazing Fast, Incremental** — Rust-powered and Blake3-hashed. Re-indexing a large project only touches files that actually changed. The rest is instant.

- **Codebase Health Checks** — `adn stats` shows which files are indexed, when they were last updated, and how many local versus external symbols are currently in the graph.

- **Zero-Config MCP Server** — One JSON block in your Claude Desktop or Cursor config and your AI gains structural tools with pagination, local-only filtering, identifier lookup, and bounded trace depth. No API keys, no cloud, no setup friction.

- **Fully Local** — Your code never leaves your machine. The knowledge graph is a single `adn.db` file in your project directory.

---

## Quick Start

### 1. Install

Choose the distribution path that matches your environment.

#### Shell Installer (Recommended)

For macOS and Linux:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/DakshBardoliwala/ADN/releases/latest/download/adn-server-installer.sh | sh
```

For Windows PowerShell:

```bash
powershell -c "irb https://github.com/DakshBardoliwala/ADN/releases/latest/download/adn-server-installer.ps1"
```

This installs a native ADN binary from the latest GitHub release. No Rust toolchain is required.

#### Homebrew

```bash
brew install dakshbardoliwala/tap/adn-server
```

#### Cargo

For Rust developers:

```bash
cargo install adn-server
```

### 2. Index Your Project

```bash
cd ~/my-project
adn index .
```

```
Walking directory ...
Indexing file "src/models/user.py"
Indexing file "src/services/auth.py"
Indexing file "src/api/routes.py"
Resolving deferred imports ...
Indexing complete!
```

### 3. Start Exploring

```bash
# Find any symbol by name
adn search authenticate

# Check what ADN has indexed and when
adn stats

# See everything a file exports
adn ls src/services/auth.py

# Inspect a symbol without looking up a UUID first
adn inspect --name AuthService --file src/auth.py

# Trace the blast radius of a change with explicit depth
adn trace --name AuthService --file src/auth.py --depth 3
```

---

## MCP Integration

Connect ADN to your AI through the installed `adn` binary.

**Prerequisites:** Run `adn index .` in your project first.

### Claude Code

```bash
claude mcp add adn -- adn mcp serve
```

Claude can also read project-shared MCP configuration from `.mcp.json`.

### Codex

```bash
codex mcp add adn -- adn mcp serve
```

Codex stores direct MCP configuration in `~/.codex/config.toml`.

### Manual JSON Configuration

Use this for clients that expect explicit JSON MCP configuration:

```json
{
  "mcpServers": {
    "adn": {
      "command": "adn",
      "args": ["mcp", "serve"],
      "cwd": "/absolute/path/to/your/project"
    }
  }
}
```

> Set `cwd` to the same directory where you ran `adn index`. That directory contains `adn.db`. If your MCP client does not support `cwd`, start it from the project root instead.

### Tools Your AI Gains

| Tool | What It Does for the Agent |
|---|---|
| `search_codebase` | Search symbols by name fragment with paginated results (`limit`, `offset`) and optional local-only filtering to exclude external module placeholders |
| `get_node_details` | Fetch node metadata plus incoming and outgoing edges by UUID or by `{ name, file_path }` |
| `list_file_symbols` | *"What's exported from this file?"* |
| `trace_impact` | Trace upstream impact by UUID or identifier lookup, with configurable bounded recursion depth |
| `list_indexed_files` | Return the indexed file inventory with `last_indexed` timestamps plus local/external symbol counts |

---

## Impact Tracing in Action

```bash
adn trace --name AuthService --file src/auth.py --depth 3
```

```
Trace Target: [class] AuthService (src/auth.py) lines 10-74 id=2f3c...
Max Depth: 3

├─ imports: [file] src/api/routes.py (src/api/routes.py) id=41ab...
│  ├─ imports: [file] src/app.py (src/app.py) id=7d20...
│  │  └─ imports: [file] src/server.py (src/server.py) id=9c51...
│  └─ imports: [file] src/jobs/sync_users.py (src/jobs/sync_users.py) id=b13e...
└─ imports: [file] src/api/middleware.py (src/api/middleware.py) id=55ef...
   └─ imports: [file] src/server.py (src/server.py) id=9c51...
```

**Read this as:** changing `AuthService` will affect the API entrypoints that import it, plus the higher-level files that depend on those entrypoints. You can widen or narrow that view with `--depth`.

---

## CLI Reference

| Command | What It Does |
|---|---|
| `adn index <path>` | Build (or update) the knowledge graph for a project |
| `adn search <query> [--limit N] [--offset N] [--local] [--json]` | Find symbols by name with optional pagination and local-only filtering |
| `adn inspect [id] [--name SYMBOL --file PATH] [--json]` | Show a node's full metadata and connected edges by UUID or human-readable identifier |
| `adn ls <path>` | List all symbols in a file, ordered by line number |
| `adn trace [id] [--name SYMBOL --file PATH] [--depth N] [--json]` | Show the upstream dependency tree for a node by UUID or identifier, with configurable trace depth |
| `adn stats [--json]` | Print the indexed file inventory with `last_indexed` timestamps and local/external symbol counts |
| `adn mcp serve` | Start the MCP server (used by Claude/Cursor config) |

All commands support `--json` for clean, pipeable output.

---

## Completed in v0.2.0

- [x] Human-readable `inspect` and `trace` lookups with `--name` and `--file`
- [x] Paginated `search` with `--limit` and `--offset`
- [x] Local-only search filtering for both CLI and MCP
- [x] Configurable trace depth for CLI and MCP
- [x] `adn stats` and `list_indexed_files` for indexed-file health checks
- [x] Process-level test harness for query and MCP regression coverage

## Upcoming in v0.3.0

- [ ] Multi-language support (Rust parser)
- [ ] Symbol-level content hashing (`blake3`)
- [ ] Web-based graph visualizer

---

## License

Apache-2.0. See [LICENSE](LICENSE).
