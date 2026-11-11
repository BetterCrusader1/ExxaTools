# ExxaTools

Manifest and depot manager for the Linux Steam client, driven by
[Vapor Forge](https://github.com/Hintay/vapor-forge).

It is the Linux answer to the Windows-style Lua app managers: pick an app,
choose its depots, pin manifests, store depot keys, and write a Lua plan into
Vapor Forge's script directory. Vapor Forge does the hooking and Steam does the
downloading.

Python 3.10+ standard library only. No pip install, no build step.

## Run

```sh
./exxa                        # local UI on http://127.0.0.1:8765/
./exxa serve --window         # same UI in a GTK/WebKit app window
./exxa serve --port 9000
./exxa engine install         # fetch, verify, and wire the Vapor Forge engine
./exxa engine status
./exxa status
./exxa search "portal"
./exxa app 730 --branch public
./exxa key 731 <64-hex-key>
./exxa code 730 731 7643209663222864337
./exxa install 730
./exxa check 730              # parse the plan with luac, the way the runtime does
./exxa fix 730                # Stop Steam, patch appmanifest flags, start Steam
./exxa provider --write       # install the request-code provider script
./exxa shortcut                # add the desktop menu entry
```

## What it talks to

| Purpose | Source |
| --- | --- |
| Store search, names, prices | `store.steampowered.com/api/storesearch` |
| App, branches, depot manifests | `api.steamcmd.net/v1/info/<appid>` |
| Manifest request codes | `manifest.steam.run`, `20770407.xyz`, `gmrc.wudrm.com`, `manifest.manifestdex.com`, `manifest.opensteamtool.com` |
| Depot decryption keys | Hubcap Manifest with a free `smm_` API key, pasted by hand, or your own `{depotId}` key source URL |

Steam will not hand out depot keys or manifest request codes for apps an
account does not own, so keys are pasted once and kept in
`~/.config/exxatools/state.json`. Codes are resolved through the same public
provider pool Vapor Forge itself uses, then cached by manifest id.

## Flow

1. **Engine first.** `./exxa engine install` downloads the current release,
   verifies its sha256, runs its installer, and wires `LD_AUDIT`. Without it
   Steam never reads a plan.
2. **Search** an app, open it.
3. Depots load for the selected **branch** with their current manifest id and
   size. Depots with no `oslist` are shared depots, not platform ones.
4. Tick the depots you need. "All Linux" is the usual starting point.
5. Store a **key** per encrypted depot (64 hex characters), or set a key source
   URL in settings and use `fetch`.
6. **Install provider** once. The generated
   `zz-exxatools-provider.lua` implements `fetch_manifest_code_ex`, so Vapor
   Forge asks the provider pool for a request code at download time instead of
   trusting a code copied into the plan weeks ago.
7. **Save plan**, then **Install to Vapor Forge**.
8. **Restart Steam**. The engine log card shows
   `scripting: scripts loaded apps=N ... manifests=M manifest_code_ex=true`
   when the plan is live.
9. After Steam finishes downloading, **Fix update flags** stops the "update
   required" / re-download loop. It closes Steam, patches
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
strings and the script normalises both forms. Codes come back as strings for the
same reason.

Vapor Forge watches this directory and hot-reloads on write. Disabling a plan
renames the file to `<appid>.lua.disabled`, which Vapor Forge ignores.

The `-- @key value` header lines are ExxaTools metadata; Vapor Forge treats
them as comments.

## Requirements on the Steam side

The engine has to be installed and wired through `LD_AUDIT`, and Steam has to be
restarted after that. `./exxa engine status` reports what it found, including
whether the *running* client actually has the shim loaded (read from
`/proc/<pid>/environ`, not guessed):

```
engine installed:  yes
version:           v0.1.2
libs:              4/4
ld_audit wired:    yes
steam running:     yes
engine in steam:   loaded
```

Manifest request codes are also configurable inside Vapor Forge
(`[manifest] providers` in `~/.config/vapor-forge/config.toml`). Keep the two
lists aligned if you add your own source; settings here accept the same built-in
names plus a custom `name`-only entry for display.

## Hubcap

[Hubcap Manifest](https://hubcapmanifest.com) hands out a free API key (`smm_…`)
and returns, per app, a zip holding a lua with the depot list, the depot keys,
and the manifest ids of the build it archived. That is the half Steam never
gives you: keys.

```sh
./exxa hubcap key smm_…          # validate and store the key
./exxa hubcap stats              # daily usage, whether you can still request
./exxa hubcap search cyberpunk   # what Hubcap actually has
./exxa hubcap fetch 1091500      # import every depot key into the store
./exxa hubcap fetch 1091500 --pin --plan   # also pin its manifests, create a plan
```

Keys always import. Manifest ids only move with `--pin`, because Hubcap archives
a snapshot and that snapshot is not necessarily the branch you selected — pinning
is a deliberate step, not a side effect of fetching. A plan is only created with
`--plan`, so a fetch never replaces a depot list you tuned by hand.

In the UI: **Settings → Hubcap API key**, then **Hubcap keys** in the app
toolbar. The daily quota is shown next to the field; Hubcap rejects cyrillic
search terms with 400 and that is upstream, not this tool.

## Steam menu icon

The client gets our mark right after **Help** in its own menu bar. Steam's UI is
Chromium, so with `-cef-enable-debugging` it exposes CDP on `127.0.0.1:8080`;
the injection clones the Help menu item (so hover and spacing stay native),
swaps its label for the gold VD mark, and hangs a panel off it that talks to
this app's local API.

```sh
./exxa steamui status     # debug port, target, whether the item is there
./exxa steamui inject     # adds it now
./exxa steamui remove
./exxa steamui enable     # writes the flag into the launcher and restarts Steam
```

In the UI: **Engine → Inject menu / Remove / Enable debug**.

Details that matter:

* The flag has to be on the client's command line, so `Enable debug` patches
  `~/.local/share/applications/steam.desktop` and restarts Steam with
  `-cef-enable-debugging`.
* A `MutationObserver` inside the page re-adds the icon when Steam re-renders
  its menu bar.
* `fetch` from the Steam page to `http://127.0.0.1:8765` works (loopback is a
  trustworthy origin); the API answers with permissive CORS headers anyway.
* The panel is plain DOM: status line from `/api/engine`, then Open ExxaTools,
  Restart Steam, Install provider, About.
* An OS-level window grab can miss the CEF overlay; `Page.captureScreenshot`
  over CDP shows what the client actually renders.

## Troubleshooting

| Symptom | Cause |
| --- | --- |
| Steam never reads the plan | engine not installed, or Steam started before `LD_AUDIT` was wired — restart it |
| Plan written, nothing unlocks | missing depot key: `verify` shows which depots have none |
| "Update required" loops after a restart | appmanifest flags; run `./exxa fix <appid>` with Steam closed |
| Download stalls on "content servers unreachable" | every request-code provider is rate limited; wait or add a source |
| Window opens dark and empty | WebKit's DMA-BUF renderer; the launcher already forces the shm path |
| `luac` missing | `pacman -S lua` for real parse checks; lint still runs |

## Layout

```
exxatools/
  http.py        urllib transport with client identity
  steam.py       store search, appinfo, depot/manifest normalisation
  providers.py   manifest request-code providers, rate limited per service
  hubcap.py      Hubcap Manifest client: stats, search, fetch, lua parse, import
  lua.py         plan rendering and the provider Lua template
  vfsys.py       Vapor Forge install detection and script directory
  engine.py      engine install/update/uninstall, plan parse, engine log
  appmanifest.py steamapps appmanifest reads and update-flag patches
  steamctl.py    Steam roots, accounts, launch options, restart, LD_AUDIT probe
  configfile.py  config.toml injection for tickets, DLC, steam id overrides
  store.py       state file: keys, plans, cached codes, settings
  server.py      JSON API + static front end
  desktop.py     GTK/WebKit window, chromium/firefox fallbacks, menu entry
web/             single-page UI
vd               launcher
```

State lives in `~/.config/exxatools/state.json`. Set `XDG_CONFIG_HOME` or
`EXXATOOLS_HOME` to move it. The engine tarball cache lives in
`~/.cache/exxatools/engine`; installed engine metadata is
`~/.local/share/vapor-forge/exxatools-engine.json`.
