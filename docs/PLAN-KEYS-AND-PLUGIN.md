# Plan: keys without file juggling, and a menu that lives in Steam

Status: **draft, awaiting acceptance**. Nothing below has been implemented.
Research basis: `docs/RESEARCH-COMPETITORS.md` (retrieved 2026-11-12).

## Goal

A person types a game name, presses one button, restarts Steam and plays. Keys,
manifest pins and the plan itself arrive without a `.lua` file and without a
second program, and the ExxaTools menu is part of the Steam client rather than
something injected over CDP.

Goal beyond the request: today the tool assumes the user already has a plan file
from a Discord community. The outcome we aim at is a tool that resolves the plan
itself and degrades to manual import only when every source refuses.

## Non-goals for this phase

- Denuvo tickets (AppTicket/ETicket), error `88500005` handling.
- Downloading depot content from the CDN without Steam.
- Online-play fixes, per-game crack libraries, DLC unlockers.
- Windows build.
- Replacing Vapor Forge as the engine.

## Assumptions

- Ryuu's documented API (`X-Auth-Key`, `/api/download/{appid}?file_type=lua`) and
  Hubcap's seven-day keys stay available; both are community services and may
  change or vanish. The design must fail visibly, not silently.
- Millennium is MIT and installs on this machine (AUR or its installer).
- Vapor Forge keeps hot-reloading `~/.config/vapor-forge/scripts/`.
- The current state file can grow: keys and provider cache live in it.

## Acceptance criteria

1. Searching a game and pressing **Get plan** produces a saved plan with at
   least one depot key and an installed script, for a title the configured
   provider can serve — no file is opened by hand.
2. When the first provider fails, the next one is tried in the configured order;
   the UI names which provider answered and the reason the others did not.
3. Provider calls respect a per-provider minimum interval and daily budget; a
   refused quota does not produce a retry storm.
4. Each depot in a plan can be switched off, and a manifest id can be pinned per
   depot; the regenerated script reflects both and the choice survives a restart.
5. The ExxaTools item appears in Steam's menu bar after a normal Steam start
   (no `-cef-enable-debugging`, no CDP) and its panel shows engine state and the
   saved plans.
6. `cargo test --release` is green, `cargo build --release` has no warnings, and
   the closing gate re-runs both.

## Verification shape

| Criterion | Command or check | Passes when |
| --- | --- | --- |
| 1 | `./exxa plan fetch 620` then `./exxa plan show 620` | plan exists, key count > 0, script contains a 64-hex key |
| 2, 3 | unit tests over recorded provider fixtures; `./exxa providers test` | fallback order honoured, interval respected, error text names the provider |
| 4 | `./exxa toggle 620 621 --off` then read the script | depot block absent, `state.json` remembers it |
| 5 | restart Steam, look at the menu bar, open the panel | item present, panel lists the plan |
| 6 | `cargo test --release`, `cargo build --release` | 0 failed, 0 warnings |

## Tasks

Each task is a vertical slice: behaviour through the surface a user reaches.

### T1 — Provider client with quotas
Depends on: none.
Writes: `src/providers.rs`, `src/state.rs`, `src/http.rs`, `tests/providers.rs` (new).
Done check: `cargo test --release providers::` green, including fixture tests for
Ryuu JSON, Hubcap `addappid` text, HTTP 429, HTML challenge, and a fallback run
that proves provider order and the 1 s interval.

### T2 — `plan fetch` end to end
Depends on: T1.
Writes: `src/main.rs`, `src/server.rs`, `src/hubcap.rs`.
Done check: `./exxa plan fetch 620` prints the provider name and the number of
keys, writes `state.json` with keys, and `./exxa plan show 620` renders a script
that contains one 64-hex key on this machine.

### T3 — One button in the UI
Depends on: T2.
Writes: `web/app.js`, `web/index.html`, `web/style.css`.
Done check: in the browser, search `portal 2`, open the game, press Get plan; the
card appears with "keys ready" and the toast names the provider; the network tab
shows a single POST to `/api/plan/fetch`.

### T4 — Per-depot editor
Depends on: T2 (same plan record), can run beside T3 but writes different files.
Writes: `src/lua.rs`, `src/server.rs`, `web/app.js`, `web/style.css`.
Done check: switch depot 621 off in the drawer, save, then `./exxa plan show 620`
no longer contains `addappid(621` and a restart keeps the toggle.

### T5 — Millennium plugin: menu and panel
Depends on: none (talks to the loopback API only).
Writes: `plugin/` (new), `src/millennium.rs` (new), `src/main.rs`.
Done check: `./exxa millennium install`, restart Steam normally, the menu item is
present and the panel shows engine state and the plan list; `grep -c cdp
src/steamui.rs` unchanged and the CDP watcher is not started for the menu.

### T6 — Closing gate
Depends on: T1, T2, T3, T4, T5.
Writes: `docs/`, plus a fix commit if the review finds anything.
Done check: a delegated verifier (not the implementing agent) runs
`cargo test --release`, `cargo build --release`, the five commands from the
verification table, and reports each result verbatim. A non-clean review adds a
fix task with its own id and done check and reruns this gate; at most two rounds.

## Review Focus

| Input the requirements imply | Expected behaviour | Owning task |
| --- | --- | --- |
| Provider returns HTML or a Cloudflare challenge | Clear "provider blocked" message, next provider tried | T1 |
| Quota refused (429) | No retry loop; the reason is shown once | T1 |
| Game absent from every provider | "No plan found" plus the manual import path | T3 |
| Depot switched off but its key exists | Script omits both `addappid` and `setmanifestid` for it | T4 |
| Steam update breaks the plugin | Window and CLI keep working; the plugin is not required to add games | T5 |

## Risks and rejected options

- **Ryuu/Hubcap terms and quotas change** — mitigated by fixture-driven tests and
  visible provider state; not mitigated by hard-coding a single provider.
- **Millennium plugin API churn** — the plugin is isolated in `plugin/`; the
  window remains the primary surface.
- Rejected: bundling Hubcap keys in the binary (they expire weekly and would be
  redistributed), scraping SteamDB (Cloudflare, ToS), vendoring SLSsteam
  (AGPL-3.0 obligations for a non-AGPL binary), and auto-installing providers'
  executables (the ecosystem's malware record).

## Open evidence gaps

- No measurement of provider success rates; the design assumes fallback is rare.
- Whether Millennium's plugin API is stable across a Steam client update is not
  observed; the plan keeps the window as the fallback.
- `omh runtime record` is unavailable in this environment (runtime binding
  failed), so runtime evidence for this plan is `not_observed`.