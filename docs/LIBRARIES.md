# Libraries: what we use, what we test, what we reject

Research for the Rust port. Every row is either a checked fact — version,
licence, dependency tree — or an explicitly marked estimate.

## What the product actually needs

1. **Steam CM** — login, licences, PICS (appinfo), depot keys. Without it we
   depend on Hubcap forever.
2. **CDN** — manifests, chunks, AES, zstd/lzma, delta patches.
3. **VDF/ACF** — read and write `appmanifest_*.acf` without dropping fields.
4. **HTTP/TLS and WebSocket** — APIs, CDP.
5. **ZIP** — Hubcap archives.
6. **Window** — currently python + webkit, we want a single binary.
7. **Steam UI** — currently CDP injection, which is brittle.
8. **Artwork** — covers for games without `library_600x900`.
9. **Lua** — validate our plans with a real interpreter.
10. **FS watch** — catch the moment Steam finishes an appmanifest.

## Crates

| Crate | Version | Licence | Gives us | Build cost | Verdict |
| --- | --- | --- | --- | --- | --- |
| **steamroom** | 0.3.0 | Apache-2.0 | CM client, QR and credential login, PICS, manifest parsing, AES chunk decrypt, CDN pooling, KeyValue binary and text | tokio + reqwest + rustls + prost + rsa, ~120 crates | **Take it, behind a feature.** The only path to "your own account" without Hubcap |
| **steamroom-client** | 0.3.0 | Apache-2.0 | The same plus retry, delta patching, progress events | same tree | Take with steamroom |
| steamdepot | 0.3.3 | **LGPL-2.1** | Same capabilities, different implementation | same tree | **Do not link.** LGPL plus static Rust means shipping relinkable objects. Reference only |
| **keyvalues-parser** | 0.2.4 | MIT/Apache-2.0 | VDF text parse and render, 458k downloads | 2 crates | **Take** for appmanifest and config.vdf |
| **steamlocate** | 2.1.1 | MIT | Finds Steam, libraries, installed apps | 3 crates | **Take.** Verified on this machine |
| **ureq** | 3.5.1 | MIT/Apache-2.0 | Blocking HTTP over rustls | ~20 crates | **Take**, instead of shelling out to curl |
| **tungstenite** | 0.30.0 | MIT/Apache-2.0 | WebSocket for CDP | 5 crates | **Take**, instead of a hand-rolled client |
| **zip** | 2.x | MIT | Hubcap archive reading | 3 crates | Take |
| **notify** | 8.2.2 | CC0-1.0 | inotify on appmanifest and Steam log | 4 crates | Take for the watcher |
| mlua | 0.12.2 | MIT | Lua 5.4 / LuaJIT from Rust | system lua or vendored | Optional: validate plans before writing |
| steamguard | 0.18.4 | MIT | 2FA codes, mobile confirmations | moderate | Only for a full own-account login |
| prost | 0.14.5 | Apache-2.0 | Protobuf for CM | rustc build step | Not separately: arrives with steamroom |
| wry + tao | 0.57 / 0.37 | MIT/Apache-2.0 | WebView window on webkit2gtk | ~60 crates | **Later.** Replaces the python helper |
| egui / iced / slint | 0.37 / 0.14 / 1.18 | MIT | GUI without a webview | 30–60 crates, but throws away our layout | No |
| steamgriddb_api | 0.3.1 | MIT | Artwork from SteamGridDB | unmaintained since 2022, async over reqwest | **No.** Three HTTP calls beat a stale dependency |
| steamworks | 0.13.1 | MIT | Official SDK bindings | needs the Steamworks SDK | No: it is for your own games, not a library manager |

## Non-crate projects worth knowing

| Project | What it is | Why it matters |
| --- | --- | --- |
| **Millennium / SteamBrew** | Steam mod loader that hooks the client's server router and patches JS **while Steam serves it** | Replaces our CDP injection: the panel becomes part of the UI instead of a DOM patch. Linux x86_64 supported. Plugin backend is Lua, frontend is TS |
| **SteamProtocolBreakdown** | 873 lines walking through login, PICS, depot keys, CDN | The spec to write our own CM client without guessing |
| **ManifestHub3** | Community depot-key cache | One more key provider |
| **ManifestDeX.Cli** | .NET CLI for keys and appinfo | A provider, not a library |
| **opensteamdb-api** | Self-hosted appinfo API | Needs Postgres and Redis; not for us |
| **SLSsteam** | C++ reference for the Linux client | Read it, do not link it |
| **vapor-forge** | Our engine, AGPL-3.0 | Loaded as a `.so` through `LD_AUDIT`, which is not linking, so the licence does not reach us |

## Measured on this machine

Minimal core (serde, serde_json, sha2, base64, zip, keyvalues-parser,
steamlocate, ureq, tungstenite, notify):

```
cargo build --release   14.4 s
target/                 179 MB
binary                  308 KB   (strip + lto + opt-level=s)
```

Separate probe with steamroom alone, same release profile:

```
cargo build --release   53.2 s
target/                 685 MB
~/.cargo/registry       +100 MB
```

So "40 GB" is a myth here. Core is 14 s and 179 MB; core plus steamroom is about
68 s and 0.9 GB, and only when the feature is on. The steamroom tree — tokio,
reqwest, rustls, prost, rsa — outweighs everything else combined, which is
exactly why it sits behind a flag.

Verified working, not just compiled: `steamlocate` found Steam at
`~/.local/share/Steam` and its library, `keyvalues-parser` parsed an `AppState`
block out of an appmanifest.

## Decision

Locked. This is the stack the port is written against.

**Core, always compiled:** `serde`, `serde_json`, `keyvalues-parser`,
`steamlocate`, `ureq`, `tungstenite`, `zip`, `sha2`, `notify`, `base64`.

**`--features steam-login`:** `steamroom` + `steamroom-client`. Gives own-account
login by QR or password, licences, PICS, depot keys, and manifest downloads
without Hubcap.

**`--features webview`:** `wry` + `tao`, replacing the python helper once the
core reaches parity.

**Steam UI:** Millennium plugin as the primary path, CDP kept only as a
development fallback. Reason: router-level patching survives Steam updates far
better than DOM selectors, and it does not fight the page for ownership.

**Artwork:** SteamGridDB over plain HTTP, no wrapper crate.

**Lua:** `mlua` as an optional validation dependency.

Why steamroom over steamdepot: same capability, Apache-2.0 instead of LGPL-2.1,
actively maintained, and the same author ships an orchestration layer with delta
patching. Licence compatibility decides this one, quality confirms it.

## Four experiments before the port (1–2 hours each)

1. **steamroom QR login** → licences → `get_depot_key(730, 731)` → compare with
   what Hubcap returns. If they match, Hubcap becomes the fallback path.
2. **Millennium plugin** with our panel instead of CDP. Check it survives a Steam
   update and does not collide with Vapor Forge's `LD_AUDIT`.
3. **notify watcher** on `appmanifest_*.acf`: auto-fix on `StateFlags=4`.
4. **steamlocate** replacing our own `libraryfolders.vdf` parser, tested against
   a library on another disk.

## What this changes

- The Rust port keeps tokio out of the core; heavy pieces live behind features.
- The Steam menu moves from CDP to a Millennium plugin.
- Keys gain an "own account → PICS" path; Hubcap stays for everything else.
- Artwork comes from SteamGridDB for games without a cover.
- `steamdepot` is read, never linked, because of its LGPL terms.