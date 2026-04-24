# AGENTS.md — visual_interception_event_window

This file governs the entire `visual_interception_event_window/` repository.

## Mission
Build `view` (Visual Interception Event Window), a **passive terminal agent dashboard** — a real-time, high-performance observability surface for monitoring local AI coding agents. `view` consumes JSON event streams and renders them into a multi-panel TUI and desktop UI without intercepting or interfering with the observed processes.

## Product Contract
- **Binary name**: `view`
- **Primary command**: `view` (starts the dashboard)
- **Role**: Passive Observer. It consumes JSON event streams and renders them into a multi-panel layout.
- **Core Restrictions**:
    - **Non-blocking**: The TUI must never block the event processing or input handling.
    - **Resource Efficient**: Must target ~60fps without high CPU usage; uses `tokio::time::interval`.
    - **Zero Flicker**: Correct use of `ratatui` double-buffer diffing; avoid manual `clear()` calls.
    - **Stateless/Passive**: It does not modify the state of other agents; it only visualizes observed state.

## Required Technical Choices
- `ratatui` for TUI layout and widgets (`view-cli`).
- `egui`/`eframe` for native desktop UI (`view-desktop`).
- `axum` for the web API and WebSocket server (`view-web`).
- `crossterm` for terminal backend and raw mode handling.
- `tokio` for async runtime and MPSC channels.
- `serde` and `serde_json` for event parsing.
- `anyhow` for application-level error handling.
- `chrono` for precise event timestamping.
- `parking_lot` for low-overhead `RwLock` across UI threads.

## Workspace Layout

```
crates/
├── view-core/     — domain state, engine, listener, event schemas (no UI deps)
├── view-cli/      — TUI frontend via ratatui + crossterm
├── view-desktop/  — desktop frontend via egui/eframe
└── view-web/      — web API + WebSocket server via axum
```

**State access pattern:**
```rust
state.registry.agents      // AgentRegistry — agent data & events
state.terminals.sessions   // TerminalManager — PTY sessions
state.ui.selected_*        // UiState — ephemeral interaction state
```

## Output Contract for AI Agents
- **Visual Excellence**: The TUI/desktop UI should be extremely premium — modern colors, bold highlights, and clear layouts.
- **JSON Compatibility**: Internal event schemas must remain compatible with any JSON-streaming agent that follows the snapshot/lifecycle event contract described in `README.md`.

## Code Quality Rules
- **Panic-free**: No `unwrap()`, `expect()`, or `panic!` macros in production execution paths. Use `?`, `if let`, or `match`.
- **Terminal Hygiene**: Use RAII guards (`Drop` implementation) to ensure the terminal is restored (raw mode off, alternate screen left) even on panics or unexpected exits.
- **Concurrency**: UI thread must remain responsive; use `parking_lot::RwLock` for state shared across threads. Never block the egui/ratatui render thread.
- **IME Disabled** (`view-desktop`): `ctx.output_mut(|o| o.ime = None)` must run every frame to prevent macOS IME interference.

## Commit and Agent-Knowledge Rules
- Treat git history as part of the agent memory for this repo.
- Every meaningful change should be committed with a Conventional Commit style subject: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`.
- For non-trivial commits, include lore-style trailers:
    - `Constraint: ...`
    - `Rejected: ...`
    - `Confidence: low|medium|high`
    - `Scope-risk: narrow|moderate|broad`
    - `Directive: ...`
    - `Tested: ...`
    - `Not-tested: ...`
- Do not combine unrelated work into one commit; preserve a searchable knowledge trail.

## Agent Behavioral Guidelines

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

### 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them — don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it — don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Always run `cargo check --workspace` before declaring a task done.

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.
