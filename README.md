# ExxaTools

Manifest and depot manager for the Linux Steam client, driven by
[Vapor Forge](https://github.com/Hintay/vapor-forge).

It is the Linux answer to the Windows-style Lua app managers: pick an app,
choose its depots, pin manifests, store depot keys, and write a Lua plan into
Vapor Forge's script directory. Vapor Forge does the hooking, Steam does the
downloading.

![ExxaTools](assets/game.png)

Rust, one binary. The native GTK4 window needs `gtk4` and `libadwaita` at
runtime. `./exxa` builds it on first use and execs it.

## Starts with Steam

Starting Steam starts ExxaTools. No scripts, no autostart entry, no
supervision loop: a wrapper around Steam's container launcher raises the
companion server before the client enters its sandbox, and the patched
Vapor Forge shim keeps the hooks alive inside it.

```sh
steam                     # ExxaTools comes up on 127.0.0.1:8765 by itself
./exxa steam             # the same, plus the CEF debug port and a health check
```

The moving parts, and why they are shaped this way:

- **Container launcher wrapper.** Steam re-extracts
  `srt-client-container` and re-verifies package file sizes on every start, so
  the wrapper is a script padded to the original's exact byte size and
  reinstalled on every launch. A Steam update replaces it with a newer stock
  binary, which the next launch adopts.
- **One audit path, not a list.** `pv-adverb` refuses a colon-separated
  `LD_AUDIT`, so the container receives only the 64-bit shim. The 32-bit
  loader ignores a 64-bit audit library, and the containerised client is
  64-bit.
- **Companion server.** The wrapper execs
  `~/.local/share/exxatools/bin/exxa serve` detached before Steam starts.
  `./exxa steam` and `./exxa serve` keep that private copy current and
  executable; the server is single-instance on its port, so a Steam restart
  reuses the running one and a second launch exits cleanly.
- **Session cookies.** The login session rotates its Xauthority files, so the
  launcher picks the cookie by newest mtime, not by name.

`./exxa steamui install` rewrites the local Steam desktop entry so the stock
icon and its `steam://` actions go through that wrapper; `uninstall` returns
the stock launcher. `./exxa steamui status` reports the whole chain: wrapper,
server, debug port and whether the menu is injected.

### The Steam client patch

Steam build `1794536281` ships a `steamclient.so` whose instruction layout
breaks most of Vapor Forge v0.1.2's semantic checks, and its runtime strips
`LD_AUDIT` before the client starts inside pressure-vessel. Both are handled
by one patch kept in [`patches/vapor-forge/`](patches/vapor-forge/):

- ownership hooks (`CUser::CheckAppOwnership`, `CUser::GetSubscribedApps`)
  resolve from the live binary even when the old validators reject the new
  layout, gated behind `VAPOR_FORGE_TRUST_PATTERNS`;
- license sync latches from the NULL/0 sizing call this build actually makes,
  instead of a fill call that never comes;
- `BuildDepotDependency` appends pinned depots for apps in the active plan —
  Steam returns an empty vector for an unowned app, which previously meant
  nothing could be pinned;
- WebSocket CM frames pass through untouched when the packet layout is
  unavailable, so login and the library keep working;
- depot injection no longer requires cm-interception, which this build cannot
  provide.

With the patch applied, an unowned app appears in the library, downloads
through the real CDN with its manifest cached in `depotcache`, and the
installed build starts with a working `SteamAPI_Init`. Verified end to end on
Project Zomboid: 49,550 files staged, 9.8 GB, full Steam API.

## Run

```sh
cargo build --release
./target/release/exxa serve            # UI and JSON API on 127.0.0.1:8765
./target/release/exxa status           # engine, Steam and key state
./target/release/exxa search "portal"
./target/release/exxa plan 620         # print the Lua plan, luac-checked
./target/release/exxa fix 620          # stop-loop repair for appmanifest
./target/release/exxa pins 620         # branch gid vs the plan, per depot
./target/release/exxa pin 620 621 1234567890   # pin an older manifest
./target/release/exxa doctor           # twelve checks with the fix for each
./target/release/exxa diagnose         # name the failure found in the Steam log
./target/release/exxa selftest         # prove the plan pipeline on a local fixture
./target/release/exxa storage          # free space for every Steam library
./target/release/exxa diagnostics      # support bundle with keys masked
./target/release/exxa watch --once     # what still needs the flag repair
```

```sh
./exxa window                 # the native GTK dashboard
./exxa                        # print the usage JSON
./exxa serve --window         # native window plus the background API
./exxa serve --port 9000
./exxa engine install         # fetch, verify, and wire the Vapor Forge engine
./exxa engine status
./exxa status
./exxa search "portal"
./exxa app 730 --branch public
./exxa key 731 <64-hex-key>
./exxa code 730 731 7643209663222864337
./exxa plan 730 --write      # write the plan into the Vapor Forge scripts folder
./exxa check 730              # parse the plan with luac, the way the runtime does
./exxa fix 730                # stop Steam, patch appmanifest flags, start Steam
./exxa provider --write       # install the request-code provider script
./exxa shortcut               # re-inject the Steam menu (hidden desktop entry)
./exxa plugins list            # installed plugins and their state
```

## Window and menu

The app lives in Steam. The panel is injected next to **Help** in the
client's own menu bar; its **Open ExxaTools** launches the native GTK4
dashboard and raises it — a real desktop window, not a browser tab or an
iframe.

`./exxa window` opens the dashboard: a three-pane Library (navigation rail,
game shelf, the selected game's inspector with its depots and plan details),
plus Discover, System and Settings. It is single-instance, so a second launch
raises the window that already exists. Deep links open a view directly:
`./exxa window --url 'http://127.0.0.1:8765/?page=settings'`.

The window has two **Nacre** themes: pure-black dark violet and light lilac
pearl, switched live from the title bar and persisted with the settings.
Opening a game's store page adds a small ExxaTools card there. It reports
whether the game is already in the library, offers **Add to library**, **Sync
manifests** and **Dashboard**, and talks to Rust over the same injection
channel as the panel, so page CSP never blocks it.

## Manifest request codes

When Steam needs a request code the account cannot obtain, the Lua provider
script asks a chain of public mirrors, tried in order. Settings edits that
chain as one list; a custom mirror joins it as a JSON entry with a name, a URL
containing `{gid}` and an optional JSON field, and saving republishes the
script through the same code path the API uses, so Vapor Forge's hot-reload
picks it up at once.

| Purpose | Source |
| --- | --- |
| Store search, names, prices | `store.steampowered.com/api/storesearch` |
| App, branches, depot manifests | `api.steamcmd.net/v1/info/<appid>` |
| Manifest request codes | `manifest.steam.run`, `20770407.xyz`, `gmrc.wudrm.com`, `manifest.manifestdex.com`, `manifest.opensteamtool.com`, plus your own mirrors |
| Depot decryption keys | Hubcap Manifest with a free `smm_` API key, pasted by hand, or your own `{depotId}` key source URL |

Steam will not hand out depot keys or request codes for apps an account does
not own, so keys are pasted once and kept in `~/.config/exxatools/state.json`.
Codes are resolved at download time and cached by manifest id.

## Flow

1. **Start Steam.** The engine wires itself; the companion server comes up on
   8765.
2. **Search** an app, open it.
3. Depots load for the selected **branch** with their current manifest id and
   size. Depots with no `oslist` are shared depots, not platform ones.
4. Tick the depots you need. **All Linux** is the usual starting point.
5. Store a **key** per encrypted depot (64 hex characters), or fetch one from
   a configured source.
6. **Install provider** once — or edit the chain later in Settings; saving
   republishes it.
7. **Save plan**, then **Install to Vapor Forge**.
8. **Restart Steam**. The engine log shows
   `scripting: scripts loaded apps=N … manifests=M manifest_code_ex=true`
   when the plan is live.
9. After Steam finishes downloading, **Fix update flags** stops the
   "update required" / re-download loop. It closes Steam, patches
   `appmanifest_<appid>.acf` (`StateFlags 4`, `AutoUpdateBehavior 1`,
   `ScheduledAutoUpdate 0`, `BytesDownloaded = BytesToDownload`), and starts
   Steam again. It refuses while a game is running.

## What gets written

`~/.config/vapor-forge/scripts/<appid>.lua`:

```lua
-- ExxaTools plan
-- @appid 730
-- @name Counter-Strike 2
-- @branch public
-- @depots 731,732

addappid(730)

-- depot decryption keys
addappid(731, 1, "…64 hex…")

-- manifest pinning
setmanifestid(731, "7643209663222864337", 54163047457)

-- DLC
addappid(1234)
```

`~/.config/vapor-forge/scripts/zz-exxatools-provider.lua` carries the provider
chain as a Lua callback:

```lua
local SOURCES = {
  { name = "steamrun", url = "https://manifest.steam.run/api/manifest/{gid}",
    kind = "json", field = "content", agent = "ExxaTools/1.0" },
  …
}

function fetch_manifest_code_ex(app_id, depot_id, gid)
  -- returns a decimal manifest request code as a string, or nil
end
```

Manifest ids exceed 2^53, so Vapor Forge hands the large ones over as decimal
strings and the script normalises both forms. Codes come back as strings for
the same reason. The `-- @key value` header lines are ExxaTools metadata;
Vapor Forge treats them as comments.

Vapor Forge watches this directory and hot-reloads on write. Disabling a plan
renames the file to `<appid>.lua.disabled`, which Vapor Forge ignores.

## Plan sources

A plan is a `.lua` with depot keys and pinned manifests. Three kinds of source
feed it:

- **Hubcap** — free key from the Hubcap Discord; the per-app zip also carries
  depot manifest blobs, which ExxaTools stores in Steam's `depotcache` so the
  updater can mount a pinned build without a CM request code.
- **Ryuu** — `X-Auth-Key` from generator.ryuu.lol.
- **Custom** — any URL containing `{appid}`, configured under `plan_sources`
  in settings; a header can be attached for token-protected mirrors.

```sh
./exxa sources                    # which plan sources are configured
./exxa plan fetch 620             # ask each source in order, store keys, write the plan
./exxa plan fetch 620 --name "Portal 2"
```

Keys always import. Manifest ids only move with `--pin`, because a source
snapshot is not necessarily the branch you selected — pinning is a deliberate
step, not a side effect of fetching. A plan is only created with `--plan`, so
a fetch never replaces a depot list you tuned by hand.

Every state save keeps the previous file under `~/.config/exxatools/backups`,
five newest kept, so an import mistake never costs collected keys.

## Diagnosis and repairs

`./exxa diagnose` reads the tail of Steam's console log, where Vapor Forge
also writes, and names what it recognises: no internet connection (a missing
licence, not the network), an update that never finishes, content still
encrypted, a disk write error, or a manifest that no longer exists. It also
reads Vapor Forge's own pattern summary, so an engine that could not patch
the client says so instead of failing quietly.

Every incident carries the repair that fits it, and the same list drives the
dashboard banner and the row inside the Steam panel:

| Incident | Action |
| --- | --- |
| no internet connection, missing licence | rewrite every installed script from the keys on disk |
| update required, missing manifest | rebuild the plan from the current catalog, or repair appmanifest flags |
| content still encrypted | fetch keys again |
| disk write error | report free space per library with `./exxa storage` |
| engine patterns missing | install the current engine release, or apply the client patch |

A line that matches no rule produces nothing. A banner can be dismissed, and
it returns only when the same line shows up again.

## Install as a package

```sh
cd packaging && makepkg -si
```

The PKGBUILD builds the release binary and installs the desktop entry, icon
and web assets under `/usr/share/exxatools`. The binary finds that directory
on its own when it is not run from a checkout.

## Troubleshooting

| Symptom | Cause |
| --- | --- |
| Steam starts, no server on 8765 | the wrapper was replaced by a Steam update — run `./exxa steam` once to reinstall it |
| Shim missing from the client | the wrapper must pass `--pv-option=--ld-audit=`; check `exxa doctor` → Engine loaded inside Steam |
| "Update required" loops after a restart | appmanifest flags; run `./exxa fix <appid>` with Steam closed |
| Download stalls on "content servers unreachable" | every request-code mirror is rate limited; wait or add your own in Settings |
| Hooks resolve, nothing unlocks | missing depot key: `verify` shows which depots have none |
| Window does not appear | launch from Steam or `./exxa window`; the window raises itself for a moment |
| `luac` missing | `pacman -S lua` for real parse checks; lint still runs |

## Layout

```
src/
  main.rs        command dispatch
  lib.rs         the same modules, exported for tests
  server.rs      loopback HTTP server and JSON API
  plans.rs       plan sources, fetch, refresh, script writing
  lua.rs         plan and provider rendering, ids, comments
  steam.rs       appinfo, catalog, manifests
  steamctl.rs    Steam process, container wrapper, companion server, launch options
  steamui.rs     CDP injection, the in-client panel
  appmanifest.rs appmanifest reading and repair
  engine.rs      Vapor Forge install and detection
  hubcap.rs      Hubcap source and depotcache manifest blobs
  providers.rs   request-code mirrors: built-ins, custom specs, rate limits
  diagnostics.rs support bundle with masked keys
  doctor.rs       twelve checks with fixes
  state.rs       keys, plans, token, backups, transactions
  ui/            native GTK4/libadwaita dashboard
patches/         the Steam-build patch for Vapor Forge
tests/           integration tests against a temporary Steam tree
assets/          injected panel script
web/             dashboard assets
packaging/       PKGBUILD and desktop entry
```

## License

MIT — see [LICENSE](LICENSE).
