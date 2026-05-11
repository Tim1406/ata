# Scheduling: Cron / Monitor / Loop

Status: **Phase 0 complete** (feature flag registered).

This document tracks the multi-phase work to add Claude Code-style scheduling
primitives (Cron / Monitor / Loop) to ata as agent-facing tools, and to evaluate
them against the existing `codex-rs/scheduler/` crate for unification.

---

## Origin

Driver: Nima (meeting 2026-05-10).

### Nima's spec (verbatim)

> Create cron, monitor agent, similar to how Claude Code has it. And then we
> want to unified with the scheduling features on ata, and if my architecture
> is better, remove that. And unified into one Rust crate. Should be able to
> turn it on and off, with example slash experimental, and if off, the same
> function ata. LLM model be able to choose between the feature, with different
> reasoning. In the TUI, easily inspect the monitor, cron and be able to view
> them, be able to kill or delete them. For the monitoring can be in JSON file
> or similar file so we know how "here are the logs for the agent..." and
> "this agent started this cron jobs...". Be careful about agent resuming
> (when the agent dies, what are we going to do with that cron job for
> example, if it's in the middle how do we handle that, after we kill, when
> we do ata resume, do we bring it back or load the state of the agent...).

### Parsed requirements

1. Build Cron + Monitor (+ Loop) tools, exposed to the LLM, similar to Claude Code
2. Unify with existing ata scheduler crate; if the new architecture is better, **delete the old one**
3. Single unified Rust crate
4. Feature-gated on/off; off → ata behaves identically to today
5. LLM picks the right tool with distinct reasoning (cron vs monitor vs loop)
6. TUI surface to inspect, view, kill, delete cron + monitor jobs
7. Structured logs (JSON or similar) — readable as "agent X started cron job Y", "here are the logs"
8. Careful resume semantics: mid-firing crash, kill+resume, agent state reload

---

## Decisions

| Decision | Choice | Reasoning |
|---|---|---|
| Session model | **B (in-session, Claude Code style)** | Tasks fire as user messages in the live agent session. Dies with CLI close. Matches Nima's "load state of the agent" intent. |
| Existing `codex-rs/scheduler/` crate | **Keep untouched during dev** | Decide deletion at Phase 6 head-to-head review, not upfront. Nima's "if better, remove" respected literally. |
| Storage | **Session journal (`.jsonl` rollout)** | Replays on `ata resume` like message history. No new SQLite. |
| Feature flag | **`Feature::Scheduling` (`Stage::Experimental`)** | Toggleable via existing `/experimental` menu. Default off. |

---

## Phase plan (~4-5 weeks total)

| Phase | Scope | Estimate | Status |
|---|---|---|---|
| 0 | Foundation: register feature flag, no behavior change | 1-2 days | **DONE** |
| 1 | Unified crate core (`CronJob`, `MonitorTask`, `LoopTask` primitives) | ~1 week | pending |
| 2 | Agent-facing tools + tool-selection eval | ~1 week | pending |
| 3 | TUI inspection (view / kill / delete) | ~3-5 days | pending |
| 4 | Resume + state semantics | ~3-5 days | pending |
| 5 | Structured JSON logs | ~2 days | pending |
| 6 | Head-to-head evaluation; decide deletion of old scheduler | ~2-3 days | pending |

---

## Phase 0 — Foundation (DONE)

### Goal
Install the on/off switch for the entire scheduling feature, registered as a
known experimental feature in ata's feature registry. No behavior change.

### What was changed

**File:** [codex-rs/features/src/lib.rs](../codex-rs/features/src/lib.rs)

1. Added `Feature::Scheduling` variant to the `Feature` enum (ATA-private group).
2. Added a `FeatureSpec` to the `FEATURES` registry with:
   - `key: "scheduling"`
   - `stage: Stage::Experimental { name: "Scheduling", menu_description: "..." }`
   - `default_enabled: false`

**File:** [codex-rs/core/config.schema.json](../codex-rs/core/config.schema.json)

Regenerated via `just write-config-schema` so `scheduling` appears as a valid
key under `[features]` in `config.toml`. (Same pattern as Tho's CI fix commit
`be12a27a3` for `reading_view`.)

### What this enables

- The `/experimental` TUI menu now lists "Scheduling" as a toggleable feature.
- Toggling it on/off currently has no visible effect — no scheduling code exists
  to read the flag yet.
- All subsequent Phase 1-5 code can gate behavior with
  `if features.enabled(Feature::Scheduling) { ... }`, which guarantees Nima's
  requirement #4 ("off → same function ata") by construction.

### What was NOT done in Phase 0
- ❌ No new crate (waits for Phase 1)
- ❌ No tools (`CronCreate`, `Monitor`, `Loop`, etc.)
- ❌ No changes to existing `codex-rs/scheduler/`
- ❌ No new slash commands (`/experimental` already handles toggle)

### Verification
- `cargo check -p codex-features` → compiles
- `cargo test -p codex-features` → all 43 tests pass
- `just write-config-schema` → schema includes `scheduling` key
- `/experimental` menu → "Scheduling" entry visible (toggle is a no-op for now)

---

## Phase 1 — Unified crate core (PLANNED)

### Goal
Stand up a new crate (`codex-scheduling` or similar) that owns the three
primitives. Gated entirely behind `Feature::Scheduling`.

### Anticipated work
- New crate skeleton with `Cargo.toml` + `lib.rs`
- Types:
  - `CronJob` (cron expression, prompt, session-scoped)
  - `MonitorTask` (background command, line-streaming back to agent)
  - `LoopTask` (model-paced wakeups via `ScheduleWakeup`-like API)
  - Shared `TaskId`, `TaskStatus`, `TaskKind`
- Hookpoints for Phase 2 to register agent tools

---

## Phase 2 — Agent-facing tools (PLANNED)

### Goal
Expose Cron / Monitor / Loop as tools the LLM can call, with descriptions
crafted so the model picks the right one for the user's intent.

### Tool selection criteria (for tool descriptions)

| User says | Tool | Why |
|---|---|---|
| "every X min" / "at 9am daily" | Cron | Fixed schedule |
| "tell me when build finishes" / "watch logs" | Monitor | Stream-driven |
| "keep checking until merged" / "stay on this until done" | Loop | Model-paced retry with termination condition |

Each tool description includes:
- "use when…" trigger phrase
- "don't use when… (points at sibling tool)"
- concrete intent → tool example

### Eval gate
20-prompt offline eval. Tool-selection accuracy must exceed 90% before
shipping.

---

## Phase 3 — TUI inspection (PLANNED)

### Goal
User can see, view, kill, delete scheduled tasks without leaving the TUI.

### Anticipated work
- New TUI panel triggered by slash command or shortcut
- Lists: running cron, active monitors, active loops
- Per-task actions: view logs, kill, delete
- Status indicator in the main TUI (count of active tasks)

---

## Phase 4 — Resume + state semantics (PLANNED)

### Goal
Handle the hard cases Nima called out: agent dies mid-job, user kills a job,
`ata resume` replay.

### Design rules
- Cron / Monitor / Loop tied to session lifetime.
- On `ata resume`: prompt user "Monitor X was interrupted — restart? (y/n)".
  No auto-resurrect.
- Mid-firing crash: state recorded in session journal. User decides on resume.
- Kill from TUI: soft-delete (logged + persisted across resume).
- "Load state of the agent": resumed session's transcript shows what tasks
  existed and their last-known state.

---

## Phase 5 — Structured JSON logs (PLANNED)

### Goal
Greppable, readable logs. Tim should be able to ask "what did agent X do
yesterday?" and get a clean answer.

### Format (draft)
```json
{
  "ts": "2026-05-11T...",
  "agent_session_id": "...",
  "task_id": "...",
  "kind": "cron|monitor|loop",
  "event": "started|output_line|completed|failed|killed",
  "msg": "..."
}
```

Stored alongside the session rollout. TUI log viewer (Phase 3) reads from this.

---

## Phase 6 — Evaluate + decide (PLANNED)

### Goal
Compare the new in-session scheduling against the existing `codex-rs/scheduler/`
crate honestly. Decide which lives, which dies, or whether both stay with
clear roles.

### Comparison matrix

| Criterion | New (in-session B) | Old (daemon) |
|---|---|---|
| Lines of code | TBD | ~3,320 |
| Survives CLI close | ❌ | ✅ |
| Resume reliability | TBD | TBD |
| LLM tool-selection accuracy | measured in Phase 2 | N/A |
| Trigger types | cron, monitor, loop | cron, interval, file_watch, http_poll, webhook |
| Maintenance burden | new code | known issues, no active users |
| Resource usage | inside ata process | separate daemon + SQLite |

### Outcomes
- **B wins outright** → delete `codex-rs/scheduler/`
- **Old wins outright** → revert Phases 0-5, keep daemon
- **Tie / each has strengths** → keep both with distinct roles (B for agent-driven, old for `ata jobs` CLI power users)

---

## Reference files

- New work hangs off: [codex-rs/features/src/lib.rs](../codex-rs/features/src/lib.rs) (`Feature::Scheduling`)
- Existing scheduler (untouched during Phases 0-5):
  - Job schema: [codex-rs/scheduler/src/job/definition.rs](../codex-rs/scheduler/src/job/definition.rs)
  - Main loop: [codex-rs/scheduler/src/engine/scheduler.rs](../codex-rs/scheduler/src/engine/scheduler.rs)
  - Runner: [codex-rs/scheduler/src/engine/runner.rs](../codex-rs/scheduler/src/engine/runner.rs)
  - CLI: [codex-rs/scheduler/src/cli.rs](../codex-rs/scheduler/src/cli.rs)
