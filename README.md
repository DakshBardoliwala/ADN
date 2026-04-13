# ADN — Architectural Discovery Navigation

### Your AI finally understands your codebase.

[![Version](https://img.shields.io/badge/version-0.1.0-blue)](https://github.com/DakshBardoliwala/ADN/releases)
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

- **Impact Radius Tracing** — Ask *"what breaks if I change this?"* and get a precise, multi-level dependency tree. The `trace` command is your pre-refactor safety check.

- **Global Import Resolution** — Correctly links `from module import ClassName` to the exact definition, even across files that were indexed in any order. No dangling references.

- **Blazing Fast, Incremental** — Rust-powered and Blake3-hashed. Re-indexing a large project only touches files that actually changed. The rest is instant.

- **Zero-Config MCP Server** — One JSON block in your Claude Desktop or Cursor config and your AI gains four new structural tools. No API keys, no cloud, no setup friction.

- **Fully Local** — Your code never leaves your machine. The knowledge graph is a single `adn.db` file in your project directory.

---

## Quick Start

### 1. Install

```bash
cargo install adn
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

# See everything a file exports
adn ls src/services/auth.py

# Inspect a node's full connections
adn inspect <id>

# Trace the blast radius of a change
adn trace <id>
```

---

## MCP Integration

Connect ADN to your AI in under 2 minutes.

**Prerequisites:** Run `adn index .` in your project first.

### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

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

### Cursor

Add to `.cursor/mcp.json` in your project root:

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

> Set `cwd` to the same directory where you ran `adn index`. That's where `adn.db` lives.

### Tools Your AI Gains

| Tool | What the AI Can Now Ask |
|---|---|
| `search_codebase` | *"Find all symbols related to authentication"* |
| `get_node_details` | *"What does `PaymentService` connect to?"* |
| `list_file_symbols` | *"What's exported from this file?"* |
| `trace_impact` | *"What breaks if I change this function?"* |

---

## Impact Tracing in Action

Run `adn search authenticate` to find the node ID, then:

```bash
adn trace a1b2c3d4-...
```

```
Trace Target: [function] authenticate (src/services/auth.py) lines 14-38
Max Depth: 3

├─ imports: [file] api/routes.py (src/api/routes.py) lines 1-82 id=e5f6...
│  └─ imports: [file] main.py (src/main.py) lines 1-40 id=c7d8...
└─ imports: [file] middleware.py (src/api/middleware.py) lines 1-35 id=9a0b...
```

**Read this as:** if you change `authenticate`, it will impact `routes.py`, `middleware.py`, and anything that depends on them — surfaced in seconds, before you touch a line of code.

---

## CLI Reference

| Command | What It Does |
|---|---|
| `adn index <path>` | Build (or update) the knowledge graph for a project |
| `adn search <query>` | Find symbols by name. Add `--json` for scripting |
| `adn inspect <id>` | Show a node's full metadata and all connected edges |
| `adn ls <path>` | List all symbols in a file, ordered by line number |
| `adn trace <id>` | Show the upstream dependency tree for any node |
| `adn mcp serve` | Start the MCP server (used by Claude/Cursor config) |

All commands support `--json` for clean, pipeable output.

---

## Roadmap — v0.2.0

- [ ] **Rust support** — Extend the parser to index Rust crates alongside Python
- [ ] **Pagination** — Handle `search` and `ls` results on very large codebases
- [ ] **Call-graph edges** — Track function *call sites*, not just imports
- [ ] **`adn export`** — Dump the full graph as JSON or DOT for external tooling

---

## License

Apache-2.0. See [LICENSE](LICENSE).
