# Agent Sandbox - Deterministic Execution Firewall

A WASI-based sandbox runtime for AI agents with file-system virtualization, tool permission gating, side-effect simulation, and diff previews.

## Overview

This is Docker for AI agents - a secure sandbox that contains and controls agent actions before they execute in the real world.

## Features

- **WASI-based Execution**: WebAssembly System Interface for sandboxed execution
- **File-system Virtualization**: Virtual filesystem with diff tracking
- **Tool Permission Gating**: Control which commands/tools agents can access
- **Side-effect Simulation**: Preview changes without executing (dry-run mode)
- **Diff Previews**: See exactly what will change before committing

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Agent Sandbox                           │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Permission  │  │   Virtual   │  │  Side-Effect        │  │
│  │   Gating    │  │  Filesystem │  │  Simulation         │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │    WASI     │  │    Diff     │  │    Command          │  │
│  │   Runtime   │  │   Engine    │  │    Registry         │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

```bash
# Build the sandbox
cargo build --release

# Run in simulation mode (preview only)
./target/release/agent-sandbox --simulate run "git commit -m 'fix: bug'"

# Run with permission checking
./target/release/agent-sandbox --allow git,curl run "npm install"

# Show diff before execution
./target/release/agent-sandbox --diff run "echo 'new content' > file.txt"
```

## CLI Commands

- `agent-sandbox run <command>` - Execute a command in sandbox
- `agent-sandbox sim <command>` - Simulate execution (preview only)
- `agent-sandbox diff <command>` - Show diff without executing
- `agent-sandbox status` - Show sandbox status
- `agent-sandbox reset` - Reset virtual filesystem
