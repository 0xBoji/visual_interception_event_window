# visual_interception_event_window (`view`)

A passive, real-time terminal dashboard for monitoring local AI coding agents.

[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](#installation)

> `view` is the operator's glass cockpit for a local agent swarm.
> It subscribes to live agent event streams, turns newline-delimited JSON into a premium multi-panel TUI and desktop UI,
> and lets you inspect who is online, what they are doing, and where errors are emerging — without interfering with execution.

---

## Table of Contents

- [What this is](#what-this-is)
- [Why it exists](#why-it-exists)
- [Who should use it](#who-should-use-it)
- [Status](#status)
- [TL;DR Quickstart](#tldr-quickstart)
- [Installation](#installation)
- [Running the dashboard](#running-the-dashboard)
- [The mental model](#the-mental-model)
- [UI layout and interaction model](#ui-layout-and-interaction-model)
- [Keyboard controls](#keyboard-controls)
- [Event ingestion contract](#event-ingestion-contract)
- [Demo mode](#demo-mode)
- [Repository layout](#repository-layout)
- [Development and verification](#development-and-verification)
- [Limitations and non-goals](#limitations-and-non-goals)
- [Roadmap](#roadmap)

---

## What this is

`view` is a **passive terminal agent dashboard** — a Cargo workspace of four crates:

| Crate | Role |
|---|---|
| `view-core` | Domain state, engine, listener, and event schemas. No UI deps. Shared by all. |
| `view-cli` | TUI frontend via `ratatui` + `crossterm` |
| `view-desktop` | Native desktop frontend via `egui`/`eframe` |
| `view-web` | Web API + WebSocket server via `axum` (LAN-accessible) |

The dashboard:

- watches live agent presence from any JSON-streaming source,
- renders a zero-flicker TUI with overview cards, roster views, drill-down panes, and event feeds,
- tracks agent activity over a rolling 50-sample window,
- highlights status, error level, branch, role, token usage, and metadata in real time,
- supports grid and focus workflows for multi-agent monitoring.

The operator-facing binary is named **`view`**. Until a crates.io publish, run locally with `cargo run -p view-cli` or `cargo run -p view-desktop`.

---

## Why it exists

Once agents are running, the next bottleneck is **visibility**.

Without a dedicated observability surface:

- you know agents are running, but not which ones are healthy,
- logs are scattered across terminals and panes,
- "busy" vs "offline" vs "stalled" becomes guesswork,
- operators end up tailing raw JSON when they should be making decisions.

`view` makes swarm observability:

- **passive** — never touches or mutates agent state,
- **real-time** — renders at 60fps from a live event stream,
- **high-density** — overview cards, sparklines, and drill-down in one surface,
- **operator-friendly** — keyboard-first, works in any terminal.

---

## Who should use it

This is a good fit if you are building or operating:

- local multi-agent AI coding systems,
- autonomous tooling that emits structured JSON events,
- operator consoles for LAN-first agent swarms,
- terminal-native demos where live system state matters,
- debugging flows where fast status inspection beats raw log tailing.

It is especially useful when you need to answer:

- "Which agents are alive right now?"
- "Which branch or project is this agent on?"
- "Where did the latest warning or error come from?"
- "Is the swarm active, or are we only seeing stale state?"

---

## Status

Current implementation includes:

- 60 FPS async render loop built with `ratatui` + `crossterm`,
- RAII terminal cleanup to restore raw mode and alternate screen on exit,
- built-in demo dataset (`VIEW_DEMO=1`) for UI iteration without a live agent stream,
- dual presentation modes: **Grid** and **Focus**,
- filter cycling across **all / busy / active / offline**,
- inline search across agent id, project, role, branch, and instance name,
- agent drill-down for role, branch, tokens, addresses, and metadata,
- recent-event feed with level-aware colors (`info`, `warn`, `error`, `success`),
- rolling activity sparklines and event buffering,
- native desktop shell (`view-desktop`) sharing the same `view-core` backend,
- web API + WebSocket server (`view-web`) for LAN remote access,
- unit tests covering listener parsing, view-state behavior, and key rendering invariants.

---

## TL;DR Quickstart

```bash
# TUI (fastest path, no external deps)
VIEW_DEMO=1 cargo run -p view-cli

# Desktop shell
VIEW_DEMO=1 cargo run -p view-desktop

# Live mode (pipe any newline-delimited JSON agent stream)
cargo run -p view-cli
```

---

## Installation

**Run in-place:**
```bash
cargo run -p view-cli
cargo run -p view-desktop
```

**Install locally from source:**
```bash
cargo install --path crates/view-cli
```

No external runtime dependencies are required. Demo mode works fully offline.

---

## Running the dashboard

### 1. Demo mode

```bash
VIEW_DEMO=1 cargo run -p view-cli
```

`VIEW_DEMO` accepts: `1`, `true`, `yes`, `on`, `demo`.

Demo mode publishes a synthetic swarm with multiple agent roles, projects, statuses, event levels, token counts, and metadata.

### 2. Live mode

```bash
cargo run -p view-cli
```

In live mode, `view` reads newline-delimited JSON from stdin (or from a configured stream source). Pipe any agent that emits snapshot/event payloads and `view` will render it.

### 3. Desktop shell

```bash
VIEW_DEMO=1 cargo run -p view-desktop
```

The desktop shell shares the same `view-core` backend as the CLI and supports multi-tab terminal sessions, directory/branch pickers, and native text selection.

---

## The mental model

1. **Subscribe to a live agent stream**
   — from stdin, a child process, or the built-in demo source.

2. **Project agent state into operator state**
   — agents become rows / tiles / drill-down targets,
   — events become live feed entries,
   — metadata becomes context for decisions.

3. **Render every frame without blocking ingestion**
   — input, state updates, and redraws are decoupled,
   — the UI stays responsive even while events continue flowing.

4. **Navigate between overview and per-agent focus**
   — use filters, search, and selection to reduce noise,
   — move from fleet health to individual diagnosis quickly.

5. **Observe — do not intervene**
   — `view` never mutates agent state,
   — it only reflects observed state and recent signals.

---

## UI layout and interaction model

### 1. Header bar
Shows current stream state (`AWAITING`, `TRACKING`, `LIVE`) plus online/busy/offline counts, total events, and current mode.

### 2. Overview cards
Four stat panels summarize:

- agent health,
- event level distribution,
- focused agent/filter context,
- latest signal source and timestamp.

### 3. Workspace area

- **Grid mode** — high-density multi-agent wall for quick scanning.
- **Focus mode** — roster on the left, activity sparkline + drill-down summary + scoped live feed on the right.

### 4. Footer help bar
Keeps the most important controls visible at all times.

The dashboard also keeps:

- a rolling **50-sample activity timeline** per agent,
- a bounded **100-event** recent-event buffer,
- metadata prioritization for `cwd`, `model`, `last_file`, `last_tool`, `messages`, and `cost`.

---

## Keyboard controls

### Global navigation

- `q` / `Ctrl+C` — quit
- `Tab` — toggle **Grid / Focus** mode
- `j` or `↓` — move selection forward
- `k` or `↑` — move selection backward
- `PageDown` — jump forward by one page block
- `PageUp` — jump backward by one page block
- `Home` / `End` — select first / last visible agent
- `f` — cycle filters: `all → busy → active → offline → all`
- `Esc` — clear search query

### Search mode

- `/` — enter search mode
- type text — filter by **agent id / project / role / branch / instance name**
- `Backspace` — delete one character
- `Enter` — exit search mode, keep query applied
- `Esc` — clear query and exit search mode

---

## Event ingestion contract

`view` understands two JSON payload shapes on stdin:

### Snapshot payload
Seeds the roster with all currently visible agents.

```json
{
  "kind": "snapshot",
  "agents": [
    {
      "id": "agent-01",
      "instance_name": "agent-01.local",
      "role": "executor",
      "project": "my-project",
      "branch": "main",
      "status": "busy",
      "capabilities": ["observe", "stream-json"],
      "port": 4100,
      "addresses": ["127.0.0.1:4100"],
      "metadata": {
        "tokens": "24000",
        "rai_level": "info",
        "log": "Task in progress"
      }
    }
  ]
}
```

### Lifecycle event payload
Signals joined / updated / left transitions.

```json
{
  "kind": "updated",
  "reason": null,
  "previous": null,
  "current": {
    "id": "agent-01",
    "instance_name": "agent-01.local",
    "role": "executor",
    "project": "my-project",
    "branch": "feature/live-feed",
    "status": "busy",
    "capabilities": ["observe", "stream-json"],
    "port": 4100,
    "addresses": ["127.0.0.1:4100"],
    "metadata": {
      "rai_level": "warn",
      "log": "Queue is backing up",
      "tokens": "24000"
    }
  }
}
```

**Metadata conventions:**

- `rai_level` → colorized level (`info`, `warn`, `error`, `success`)
- `log` → human-facing event payload
- `tokens` → parsed into numeric token count for drill-down panels
- `rai_component` → optional source label (free-form string)

If fields are absent, the listener falls back to safe defaults.

---

## Demo mode

Demo mode is for interface development, screenshots, and operator rehearsals.

It simulates:

- multiple agents with mixed statuses (`busy`, `idle`, `offline`),
- rotating event levels,
- rolling activity sparkline data,
- realistic metadata: token counts, model names, file paths, and per-agent cost.

Useful when you want deterministic visual states without creating real agent traffic.

---

## Repository layout

```
visual_interception_event_window/
├── AGENTS.md
├── Cargo.toml
├── crates/
│   ├── view-core/      — shared runtime, state, engine, listener
│   ├── view-cli/       — ratatui TUI surface
│   ├── view-desktop/   — egui native desktop shell
│   └── view-web/       — axum web API + WebSocket
└── docs/
```

---

## Development and verification

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
cargo check --workspace --all-targets
```

For quick UI checks:

```bash
VIEW_DEMO=1 cargo run -p view-cli
VIEW_DEMO=1 cargo run -p view-desktop
```

The test suite covers:

- agent/event summary calculations,
- filter + search visibility behavior,
- selection and grid paging,
- view-mode toggling,
- listener metadata mapping,
- demo-mode truthy parsing,
- rendering invariants for the live feed and multi-agent grid.

---

## Limitations and non-goals

- No control plane — cannot send commands back into agents.
- No persisted event history beyond the in-memory recent buffer.
- No configurable theming or layout presets (yet).
- No alert routing or notification fan-out.
- Optimized for **fast local situational awareness**, not telemetry warehousing.

---

## Roadmap

- [ ] Rename/publish the operator-facing binary cleanly as `view`
- [ ] Add CLI flags for demo/live mode without env vars
- [ ] Support pluggable event source adapters (stdin, socket, file tail)
- [ ] Add richer aggregation panels for project-level and branch-level hot spots
- [ ] Introduce persistence/export for recent events and session snapshots
- [ ] Add screenshot/demo automation for release docs and regression review
