# Hardening program (adopted from the 0.3 audit)

The audit that drives this work lives in `docs/ROADMAP-0.3.uk.md` (supplied by the
operator, kept verbatim) with its concept images under `docs/assets/`. This file is
the English working copy: what landed, what is still open.

## Phase 0 — security and state integrity

| Item | State |
| --- | --- |
| Public settings model without secrets | done — `/api/settings` returns `has_hubcap_key` / `has_ryuu_key`, never the values |
| No raw depot keys in ordinary responses | done — `/api/app` exposes `has_key`, `/api/key` (GET and POST) returns presence only |
| Allowed Origin/Host for the local UI | done — writes accept `127.0.0.1:<port>` and `localhost:<port>` only |
| Session token for mutating calls | done — `/api/session`, header `X-Exxa-Token`, `authorize()` covered by tests |
| Steam CEF panel kept working | done — the panel never talks to HTTP; clicks travel over CDP |
| No wildcard CORS | done — no `Access-Control-*` header is emitted |
| No state changes through GET | audited — GET routes read state or run read-only checks |
| `700` state dir, `600` state and backups | done — enforced on every save, asserted in a test |
| No keys or tokens in logs or errors | done for the diagnostics bundle (every string masked, 64-hex scan in tests); hubcap error text still needs an audit |
| State read-modify-write guarded | partial — `save` takes a process-wide mutex and uses a unique temp file, but read-modify-write across two `load` calls is still not transactional |
| Body limit and Content-Type | done — 413 above 1 MiB, 415 for non-JSON bodies |

One deliberate exception: the engine digest (`engine.sha256`) stays readable in
`/api/engine` and `/api/library`. It is a public build checksum, not a secret, and the
update path compares it. `diagnostics.json` still masks it so a pasted bundle carries
no long hex runs.

Tests behind these claims: `tests/http_security.rs` (secrets, permissions, 403/200,
415, 413), `tests/api_auth.rs` (the predicate), `tests/diagnostics.rs` (hex scan).

## Phase 1 — correctness

| Item | State |
| --- | --- |
| Disabled depots are not counted as needing keys | done — `state::app_summary` filters on `enabled`, unit test added |
| DLC cache keyed by `(appid, country)` | open |
| Discover cache keyed by country | open |
| Distinct states: no plan / keys missing / ready / disabled | open |
| Drawer ignores a late response for a previously opened game | open |
| `osarch` respected together with `oslist`, with a reason per depot | open |
| Engine detection requires every library for both arches | open |
| One status model for doctor, verify, library badge and panel | open |
| Bounds on app id, depot id, manifest id and imports | partial — digits and u64 range enforced in `lua.rs` |

## Phase 2 — design system and navigation

Navigation and the page shell are in: Library, Discover, System and Settings are views
behind one nav, Settings is a page instead of a modal, System carries the engine and
disk cards, the quick actions and the check list. The palette and the shared badge,
card, modal, toast and check row already existed and are reused. Layout was checked at
1080, 900 and 620 pixels with no horizontal overflow, `prefers-reduced-motion` is
honoured, and the nav is a row of buttons reachable by Tab.

Visual QA at 1440x900 and 390x844, every view rendered: no horizontal overflow
(scrollWidth equals innerWidth at 1440, 900, 620 and 390), phone top bar, nav, search
and Settings fields fit without clipping. One defect found and fixed: the Quick
actions title "Repair all manifests" wrapped to two lines and pulled its card out of
line with the other four; the label is now "Repair manifests" and all five measure one
line.

Keyboard pass done: a card is a focusable button with an aria-label, Enter or Space
opens the drawer, focus moves to the drawer's close button, and Escape closes it and
returns focus to the card that opened it. Still open: folding the remaining ad-hoc
styles into named components.

## Not in this program

The audit explicitly keeps Rust and native GTK; no web renderer, no SPA framework, no
state-format change without a migration, and no security refactor mixed into a
cosmetic redesign.