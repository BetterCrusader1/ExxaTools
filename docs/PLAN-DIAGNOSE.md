# Plan: turn failures into fixes

Status: **draft, next to run**. Written after the self-test/backup batch.
Evidence base: `docs/RESEARCH-COMPETITORS.md`, the fixes every competitor ships.

## Goal

When a game does not work, the app says which gate failed and offers the exact
repair, instead of leaving the user with Steam's generic error and no next step.

## Non-goals

- Denuvo tickets, online play, depot downloads from the CDN.
- A per-game fixes catalogue imported from third parties.
- Anything that needs root.

## Acceptance criteria

1. A known failure line in the Steam or engine log is matched to a named
   incident with a proposed action, within one refresh of the Log panel.
2. Incidents appear in the game drawer when the line mentions its app id, and in
   a page-level banner otherwise.
3. Each proposed action runs the existing fix and reports its result.
4. Unknown lines produce no incident, and the matcher never throws on binary or
   empty logs.
5. `cargo test --release` and `./exxa selftest` stay green.

## Verification shape

| Criterion | Check | Passes when |
| --- | --- | --- |
| 1 | unit tests over recorded log fixtures | each signature maps to the expected incident id |
| 2 | browser: drawer on a game with a matching fixture line | incident row visible with a Run button |
| 3 | click Run | the fix endpoint answers and the toast shows its message |
| 4 | fixtures with random bytes and an empty file | no incident, no panic |
| 5 | `cargo test --release`, `./exxa selftest` | 0 failed |

## Tasks

### T1 — Signature matcher
Writes: `src/diagnose.rs`, `src/doctor.rs`, `tests/diagnose.rs`.
Signatures: "No internet connection", "Update required", "content still
encrypted", "Disk write error", "app state is 0x…", Vapor Forge "license" and
"depot key" failures. Each maps to `{id, title, action, detail}`.
Done check: `cargo test --release diagnose` green over six fixtures plus the
garbage and empty cases.

### T2 — Log endpoint
Writes: `src/server.rs`, `src/main.rs`.
Depends on: T1.
Done check: `GET /api/diagnose` returns incidents from the live engine log;
`exxa diagnose` prints the same JSON.

### T3 — Surface in the UI
Writes: `web/app.js`, `web/index.html`, `web/style.css`.
Depends on: T2.
Done check: with a fixture line appended to the engine log, opening the drawer
shows the incident and Run triggers the mapped fix.

### T4 — Closing gate
Depends on: T1, T2, T3.
Done check: a separate verifier runs the tests, the self-test, the endpoint and
the UI check, and a review that is not clean adds a fix task and reruns.

## Risks

- Log formats change between Steam versions: signatures are matched on stable
  substrings, and an unmatched line is silence, never a guess.
- A signature that fires on a harmless line would train users to ignore it;
  every rule needs a fixture proving the false-positive case stays quiet.

## Open

- T5 of the previous plan (Millennium plugin) stays blocked: the Millennium
  package on this machine is missing its libraries and reinstalling needs root.
  The CDP menu with its watcher remains the integration until that changes.