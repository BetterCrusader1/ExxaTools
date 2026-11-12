# ExxaTools 0.3 — one flow, provable end to end

Status: draft, awaiting acceptance. Nothing here is started until the user accepts.

## Goal beyond the request

A person with Steam installed, no terminal and no context opens ExxaTools, finds a
game, gets a plan, and ends up with the game downloading in the Steam library; when
it does not work, the same window says which step failed and offers the button that
fixes it. The plan aims at that outcome, not at a list of features: every task is
judged by whether that person gets further.

## Where the product stands today

- Rust binary, 27 modules, 61 JSON endpoints, 72 unit tests inline, zero clippy
  warnings, CI on push.
- Working: engine install, plan rendering, keys, manifests, Hubcap import, plan
  sources, wizard, drawer with depots and DLC, library, Discover, diagnose, doctor,
  verify, self-test, state backups, Steam menu panel over CDP, GTK window.
- Weak spots this plan attacks: nothing exercises the real filesystem end to end;
  the HTTP API trusts any origin and needs no token for writes; the panel shows
  alerts but no per-game actions; the appmanifest patch has never run on a file
  Steam actually wrote; refresh depends on Steam being reachable; diagnostics
  cannot be exported; static file serving has no traversal test.

## Non-goals

- Millennium plugin, tray icon, Windows support, `steamroom` login flow.
- Automatic game updates or depot downloads outside Steam.
- Any change to the Vapor Forge Lua contract.
- Copying LuaTools' visual design; this stays its own product.

## Assumptions and known facts

- Steam lives at `~/.local/share/Steam` on this machine; other machines may differ,
  so tests must not read the real tree.
- `EXXATOOLS_HOME` already redirects state; Steam and Vapor Forge paths have no
  override yet, and integration tests need one.
- A binary crate cannot be imported by `tests/`; a library target is required first.
- The Steam log and `localconfig.vdf` are the raw material for appmanifest and
  launch-option work; tests use copies, never the live files.

## Tasks

Ids are stable. Every task names its dependencies, the files it may write and the
check that proves it done. Code tasks are vertical slices: behavior is exercised
through the surface a user or caller reaches, not through a layer.

### T1 — library target so tests can import the product

Depends on: none. Writes: `src/lib.rs`, `src/main.rs`, `Cargo.toml`.
Move the module list into `src/lib.rs`, keep `main.rs` a thin dispatcher.
Done check: `cargo build --release` and `cargo test --release` both pass with the
unit suite unchanged, and `tests/import.rs` calls `exxa::lua` and passes.

### T2 — a Steam tree the tests own

Depends on: T1. Writes: `src/paths.rs`, `src/steamctl.rs`, `src/vfsys.rs`,
`tests/support/mod.rs`.
Add `EXXATOOLS_STEAM_ROOT` and `EXXATOOLS_VF_HOME` overrides next to the existing
`EXXATOOLS_HOME`, and a helper that builds a temporary tree: `steamapps/libraryfolders.vdf`,
`steamapps/appmanifest_620.acf`, `userdata/12345/config/localconfig.vdf`,
`vapor-forge/scripts`, `vapor-forge/config.toml`.
Done check: `cargo test --test fixture` prints nothing but asserts that the Steam
root, the scripts dir and the state file all resolve inside the temporary tree.

### T3 — plan pipeline end to end, without the network

Depends on: T2. Writes: `src/plans.rs`, `tests/plan_flow.rs`.
A test serves one `.lua` from a `TcpListener` on a random port as a plan source,
fetches a plan for 620, and walks the result through to an installed script.
Done check: `cargo test --test plan_flow` asserts one key stored, the script on disk
with the key line, and `verify` reporting `script`, `keys` and `lua` as ok.

### T4 — appmanifest repair on a real file

Depends on: T2. Writes: `src/appmanifest.rs`, `tests/manifest_fix.rs`.
Run the repair against the fixture `appmanifest_620.acf` in the states Steam leaves
behind: interrupted update, `StateFlags` 4 with a partial byte count, missing keys.
Done check: `cargo test --test manifest_fix` asserts `StateFlags` 4, matching byte
counters, a `.bak` with the original bytes, and an unchanged file after a second run.

### T5 — refresh without Steam online

Depends on: T3. Writes: `src/plans.rs`, `src/steam.rs`, `tests/refresh_offline.rs`.
Store the catalog snapshot with the plan when it is fetched; refresh rebuilds from
that snapshot when the Steam API does not answer.
Done check: `cargo test --test refresh_offline` sets `EXXATOOLS_OFFLINE`, refreshes a
saved plan, and asserts the report counts one snapshot, the depot gid becomes the
snapshot's, the disabled flag survives and the script carries the new pin.

### T6 — a token and an origin check for writes

Depends on: T1. Writes: `src/server.rs`, `src/state.rs`, `web/app.js`,
`tests/api_auth.rs`.
Generate a token on first run, keep it in state, require it on POST/PUT/DELETE,
reject a write whose `Origin` is present and not `http://127.0.0.1:8765`, and have
the UI send the token on every call.
Done check: `cargo test --test api_auth` covers the rule (GET open, write without or
with a wrong token refused, foreign origin refused, local origin accepted), and a
live pass on the running server shows POST 403 without the token and 200 with it.

### T7 — the drawer finishes the job

Depends on: T3. Writes: `web/app.js`, `web/style.css`, `src/server.rs`,
`tests/launch_options.rs`.
Add the per-game toggles the roadmap still lists as open: `-onlinefix` launch
option, and key coverage per depot as `18 of 20 keyed` with the missing ids listed.
Done check: `cargo test --test launch_options` sets, reads and clears the flag on a
fixture `localconfig.vdf`, checks the merge helper, and a browser pass shows the
drawer's `N of M depots keyed` line and the `-onlinefix` row.

### T8 — the Steam panel acts, not just reports

Depends on: T6, T7. Writes: `assets/steam-menu.js`, `src/steamui.rs`,
`src/server.rs`.
Each saved plan row (up to eight) gains its own actions: install or sync the plan,
rebuild that one plan, and toggle `-onlinefix`. The global actions and the alert block
stay as they are.
Done check: over CDP the panel shows the plan row with its three actions, clicking
Install runs `plans::write_script` and reports `plan <id> installed`, and a plan
removed in the dashboard disappears after the next refresh.

### T9 — a support bundle that never carries keys

Depends on: T3. Writes: `src/diagnostics.rs`, `src/server.rs`, `src/main.rs`,
`web/app.js`, `tests/diagnostics.rs`.
`exxa diagnostics > diagnostics.json` and a Settings button: doctor, verify for
every plan, versions, engine state, and the last 200 log lines with keys redacted.
Done check: the test greps the bundle for any 64-hex sequence and finds none, while
the expected sections are present and parse as JSON.

### T10 — static serving cannot escape its directory

Depends on: T1. Writes: `src/server.rs`, `tests/static_paths.rs`.
Reject `..`, absolute paths and symlinks that leave the web root, and serve `.lua`
with a text type.
Done check: `cargo test --test static_paths` asks `static_file` for `../secret.txt`,
absolute paths and a planted symlink; each returns `None`, which the server answers as
404, while `/` serves the index.

### T11 — documentation that matches the binary

Depends on: T6, T9. Writes: `README.md`, `ROADMAP.md`, `scripts/docs-check.sh`,
`Cargo.toml`, `packaging/PKGBUILD`, `CHANGELOG.md`.
Rewrite the roadmap's state table for 0.3, drop every command the binary no longer
has, bump to 0.3.0, and add a script that checks each `exxa …` command in the docs
against `exxa --help`.
Done check: `scripts/docs-check.sh` (a wrapper over `scripts/docs-check.py`) exits 0,
`cargo test` and CI stay green, and `exxa version` prints 0.3.0.

### T12 — closing gate

Depends on: T1–T11. Writes: nothing.
A reviewer other than the agent that built the work runs `cargo test --release`,
`cargo clippy --release`, `node --check web/app.js`, `exxa selftest`,
`scripts/docs-check.sh`, `exxa diagnostics` scanned for 64-hex runs, and a smoke pass
on the real Steam: menu present, panel
opens, one plan installed, dashboard shows it.
Done check: every command green and the smoke pass observed in a screenshot or CDP
output. A review that is not clean adds a fix task with its own id and done check,
then reruns this gate; at most two fix rounds, after which open findings are
reported instead of claimed as fixed.

## Review Focus

Inputs the requirements imply that no planned test covers, likeliest first:

1. Steam not running at all — panel and menu actions must fail with a readable
   message, not a hang (T8).
2. Two ExxaTools windows on different ports sharing one state file — concurrent
   save must not lose a plan (T6).
3. A plan whose catalog snapshot predates a depot removal — refresh rebuilds from the
   snapshot, so a depot missing there is dropped (T5).
4. Names with right-to-left or emoji characters in the panel row (T8).
5. A machine with no `luac` installed — verify must downgrade to a warning (T3).

## Acceptance criteria

- `cargo test --release` covers the plan pipeline, appmanifest repair, refresh,
  API auth, launch options, diagnostics and static serving, and passes.
- A write without the token is refused; the dashboard still works.
- The panel performs at least three real actions per plan.
- `exxa diagnostics` produces redacted JSON.
- Docs commands all exist; version is 0.3.0.
- The smoke pass on the real Steam is observed, not described.

## Verification shape

Every done check names an outside judge: `cargo test`, `cargo clippy`, `node --check`,
a CDP query against the running client, `scripts/docs-check.sh`, or a reviewer
subagent. No task counts as done on the author's reading of its own output.

## Risks and countermeasures

- CDP injection breaks after a Steam update — panel actions reuse the same endpoints
  as the dashboard, so a broken panel costs actions, not data (T8).
- Tests grow their own fake Steam that drifts from the real one — the fixture mirrors
  only files the product reads, and T12 smokes the real client.
- Token handling breaks the UI — the token is added in one fetch wrapper, and T6
  checks one real write before T8 depends on it.
- Snapshot refresh keeps stale data — the report names the snapshot date, and a
  manual rebuild stays available (T5).

## Order and size

T1 → T2 → (T3, T10 in parallel) → (T4, T5, T6) → (T7, T9) → T8 → T11 → T12.
Roughly two focused days of work; every task is shippable on its own and leaves the
binary green.