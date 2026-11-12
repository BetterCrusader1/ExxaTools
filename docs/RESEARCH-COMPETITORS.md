# Landscape: Steam depot and manifest tooling

Research date: 2026-11-12. Sources are listed at the end with what each one
contributed. Figures are copied from the source (stars, build sizes, limits) and
are marked as reported, not measured here. Nothing in this document was verified
by running the competing tools.

## 1. The mechanism everyone shares

Steam's download path has three separate gates, and the tools in this space
attack them one at a time:

1. **Ownership** decides whether a download button exists. Tools bypass the check
   in the client (a hook or a memory patch) and write the depot ids they want
   into the client's own configuration, so Steam itself starts the download.
2. **Content** needs a manifest id and, for encrypted depots, a 64 hex depot key.
   Lua scripts carry both: `addappid(depot, 1, "key")` and
   `setmanifestid(depot, "gid", size)`. A bypassed client writes the decryption
   key in plain text into its config, then asks the CDN, which does not check
   ownership per request.
3. **Runtime** is a different problem: SteamStub (a per-binary DRM wrapper),
   Steamworks API calls (achievements, cloud, multiplayer) and Denuvo tickets.
   Steamless removes SteamStub, Goldberg emulates the Steam API offline, and
   Denuvo needs real AppTicket/ETicket data that community `.lua` files usually
   do not contain (missing tickets surface as error `88500005`).

On Linux the same three gates exist, but the ownership gate is attacked
differently: there is no injectable DLL, so tools use `LD_AUDIT` shims and
patched client libraries instead.

## 2. Players, by layer

### 2.1 Windows desktop clients

**LuaTools** (`madoiscool/LuaTools`, MIT, 936 stars, active). WPF on .NET 8.
Browses and installs manifest sources, edits the `stplug-in` lua files with
per-depot enable/disable and depot pinning, manages "unlocker modes", injects a
companion plugin into Steam's store pages, ships 29 translations and updates
itself with Velopack. Downloads depot content through DepotDownloaderMod. Its
injection API is a polyfill of Millennium's when Millennium is absent. This is
the reference product for the whole genre and the one the original request named.

**SteamTools** (`steamtools.app`). Closed source, free download; ships a Windows
build (v2.86, 14.5 MB) and a Linux build (v2.85, 10.1 MB). Sells "premium" on a
separate shop with higher limits and unlimited downloads, and runs ManiLua, a
manifest/lua aggregator that lists SteamTools.app, DepotBox, Hubcap and Ryuu as
providers. Claims one-click fixes for "thousands of titles". Its signup flow and
telemetry are not inspectable.

**OpenSteamTool** (`OpenSteam001/OpenSteamTool`, GPL-3.0, 2605 stars, active).
Open-source Steam unlocker with Lua scripts read from `Steam/config/lua`. Recent
releases add Denuvo compatibility work (save/SteamID mismatch fixes, "automatic
Denuvo authorization sharing for legitimate accounts") and optional DLL injection
into game processes. A separate small project, `OpenSteamToolDesktop` (29 stars),
wraps it in a modern Windows desktop UI.

**SteaMidra / SFF** (`Midrags/SFF`, 2024 stars, active). The widest feature set in
the ecosystem: manifest handling, Lua, LumaCore deployment, multiplayer fix
(search on online-fix.me), a fixes library pulled from the CrakFiles repo,
SteamStub removal (Steamless), DLC unlockers (SmokeAPI, CreamAPI, Uplay),
gbe_fork-based cracking, backups, multi-language GUI, and Linux support through
SLSsteam id management. Its README warns that antivirus products flag the packed
binary and asks users to add an exclusion.

**Steam-auto-crack** (`SteamAutoCracks/Steam-auto-crack`, MIT, active). Not a
licence tool: it automates SteamStub unpacking with Steamless and Goldberg
application, producing a crack-only archive from a legitimate copy. Adjacent, but
it defines what "fix" means for the DRM layer.

### 2.2 Linux clients and client mods

**SLSsteam** (`AceSLS/SLSsteam`, AGPL-3.0, 529 stars, very active, 1083 commits).
A Steam client modification for Linux, not a lua tool: it injects through
`LD_AUDIT` (`library-inject.so` + `SLSsteam.so`), fakes app ids, bypasses family
sharing limits, fixes playtime and multiplayer presence for non-Steam games.
Ships Arch packages, a Nix flake, Flatpak support and a `setup.sh` installer.
It also maintains a public "Hall of Shame" of resellers who repackage its work.

**ACCELA / Enter the Wired** (`ciscosweater/enter-the-wired`, MIT, 374 stars).
Installer glue: one command installs ACCELA + SLSsteam, or SLSsteam plus Lua
plugins. It is the layer that makes SLSsteam usable by people who do not read
wikis, and it is what the LuaTools Linux port depends on.

**LuaTools Linux** (`Star123451/LuaToolsLinux`, 114 stars, **archived**, last
push 2026-06-16). The direct analogue of our product: a Millennium plugin with a
Python backend, SLSsteam + ACCELA integration, a one-command installer, and a
fixes menu (purchase error, missing keys, missing licence info, `fix-deps`),
plus full uninstall. It was the only real Linux LuaTools port and it is no longer
maintained. This is the open lane we are in.

**Millennium** (`SteamClientHomebrew/Millennium`, MIT, 4614 stars, active). The
plugin framework for the Steam client on Windows and Linux; the injection API
that LuaTools polyfills, and the natural replacement for our CDP-based menu.

**creamlinux** (`anticitizn/creamlinux`, `clamor-s/creamlinux`, MIT). CreamAPI's
Linux equivalent for DLC ownership; both repos are stale (last push 2022).

### 2.3 Manifest and key providers

These are the actual supply chain. All of them serve `.lua` files and/or manifests
and most require a Discord login plus a daily quota.

| Provider | Access | Reported limits | Notes |
| --- | --- | --- | --- |
| Hubcap | Discord login, API key | ~25/day; keys valid 7 days | Oldest, most used, optional premium |
| Ryuu | Discord login, `X-Auth-Key` API | ~50/day | Documented API: download by appid, `file_type=lua|manifest`, branch support, request-update endpoints |
| Luie | no key required | not stated | Pulled manifests without an API key per LuaTools wiki |
| DepotBox | listed in ManiLua | not stated | Aggregated by SteamTools.app |
| SteamTools.app | bundled with the client | premium tier | First-party provider for their own tool |

**ManifestHub** (`SteamAutoCracks/ManifestHub`, 2252 stars). A manifest cache with
an API (`/manifest?apikey=…&depotid=…&manifestid=…`); free keys last 24 hours.
Maintainers threaten to stop updating if the cache is resold.

**DepotDownloader** (`SteamRE/DepotDownloader`, GPL-2.0, 3526 stars) and
**DepotDownloaderMod** (`SteamAutoCracks/DepotDownloaderMod`). SteamKit2-based
downloaders that pull depot content straight from the CDN, with or without an
account. This is how tools get files onto disk without Steam's client.

## 3. Feature comparison

Legend: yes / no / partial. "ExxaTools" is the current tree, not a plan.

| Capability | LuaTools (Win) | LuaTools Linux | SteamTools | OpenSteamTool | SteaMidra | ExxaTools |
| --- | --- | --- | --- | --- | --- | --- |
| Native desktop UI | yes (WPF) | yes (web UI in plugin) | yes | yes (desktop wrapper) | yes | yes (GTK+WebKit) |
| Add game by search | yes | yes | yes | yes (lua files) | yes | yes |
| Lua import | yes | yes | yes | yes | yes | yes |
| Per-depot enable/pin | yes | partial | partial | no | yes | no |
| DLC selection | yes | partial | partial | partial | yes | yes |
| Key lookup built in | Hubcap/Luie/Ryuu | Hubcap | own + aggregated | no | yes | no |
| Denuvo tickets | no | no | partial | yes | partial | no |
| Online/multiplayer fix | no | no | fixes | no | yes | no |
| Depot download from CDN | yes (DDMod) | no | partial | no | yes | no |
| Fix stuck update/manifest | yes | yes | yes | yes | yes | yes |
| Steam client integration | plugin | Millennium plugin | native | config only | native | CDP menu + panel |
| Linux support | no | yes | yes | no | partial | yes |
| Packaging | installer | script | portable | installer | installer | PKGBUILD + desktop entry |
| License | MIT | none stated | closed | GPL-3.0 | not asserted | — |

## 4. Where ExxaTools stands

What we already have that the Linux side does not:

- A single Rust binary with its own loopback API, so the UI, the CLI and the
  Steam menu are one process; LuaTools Linux needed Python plus Millennium plus
  SLSsteam plus ACCELA.
- Doctor with twelve checks and one-click fixes, manifest repair, launch-option
  editing and cloud-sync reset — none of the Linux tools expose these.
- A dashboard that a non-technical person can use, which was the explicit ask.

What we lack against every serious competitor:

1. **A key source.** No provider integration, so encrypted depots only work if
   the user imports a `.lua` by hand. LuaTools pulls from Hubcap, Luie and Ryuu;
   SteamTools aggregates four providers.
2. **Per-depot control.** No way to disable a depot, pin a manifest or see which
   depot has a key inside the UI.
3. **Denuvo path.** No AppTicket/ETicket handling and no detection of error
   `88500005`.
4. **Content download.** We tell Steam to download; we cannot fetch a depot
   ourselves when Steam refuses.
5. **Client integration durability.** Our menu is CDP-injected; every competitor
   on Linux uses a Millennium plugin or a client mod.
6. **Fixes library.** SteamTools and SteaMidra ship per-game fixes; we ship
   generic repairs only.

## 5. Risks worth naming

- **Malware in the ecosystem.** Public sandboxes flag a "LuaToolsInstaller.exe"
  with malicious activity and rate `lua.tools` as moderate/high risk; SLSsteam
  keeps a scam list because resellers repackage its binaries. Anything we
  recommend as a provider is a trust decision, not just an API call.
- **Account risk.** Fake licences and online play through a patched client leave
  records on Steam's side; Denuvo "authorization sharing" is the riskiest
  feature in the space.
- **Licence obligations.** Vapor Forge and SLSsteam are AGPL-3.0; Millennium and
  LuaTools are MIT. Borrowing SLSsteam code into a non-AGPL binary is not an
  option.
- **Supply-chain fragility.** Providers are Discord-gated, rate-limited and
  sometimes disappear; ManifestHub already threatens to stop updating.

## 6. Plan feed

Ordered by what unlocks the most, given the current tree.

**P0 — Millennium plugin instead of CDP.** LuaTools Linux proved the route on
Linux and Millennium is MIT and actively maintained. This removes the watcher
loop, the "menu disappeared after a Steam update" class of bugs, and gives us a
real router to hang UI on.

**P0 — provider layer with quotas.** Ryuu has a documented API (`X-Auth-Key`,
download by appid, lua or manifest); Hubcap keys last seven days; Luie and
DepotBox fill gaps. Per-provider backoff, cache and a visible "source" picker,
matching what we already show in Settings.

**P1 — one button: "Get plan".** For a searched game, fetch the `.lua` from the
best available provider, import keys and pins, create the plan, install. This is
the whole LuaTools flow in one action and it is the single biggest gap.

**P1 — per-depot editor.** Enable/disable, pin a manifest id, show key state.
The data model already stores per-depot keys and gids.

**P2 — Denuvo handling.** Detect `88500005` in the log, expose AppTicket/ETicket
fields (`setappticket`, `seteticket`), and state the account risk in the UI
before the user pastes anything.

**P2 — download without Steam.** A DepotDownloaderMod-style CDN fetch for cases
where the client refuses, using SteamKit2 through the existing Rust HTTP layer or
by shelling out to the .NET tool.

**P3 — fixes library and online play.** A per-game fixes index (SteamTools and
SteaMidra both have one) and a search that opens online-fix results, kept
separate from the manifest flow.

## 7. Unverified and open

- SteamTools' Linux binary was not inspected; its mechanism is unknown.
- Whether LuaTools Linux being archived means the plugin API broke, or the
  maintainer simply stopped, is not established.
- Provider quotas and terms change without notice; the numbers above are what
  the sources state today.
- Our CDP menu has not been observed across a Steam client update, so the
  "fragile" claim is reasoning from the mechanism, not a measurement.

## Sources

Retrieved 2026-11-12.

- https://github.com/madoiscool/LuaTools — LuaTools features, stack, DepotDownloaderMod, Millennium polyfill, MIT, 936 stars.
- https://github.com/Star123451/LuaToolsLinux — Linux port, Millennium + SLSsteam + ACCELA, fixes menu, archived, 114 stars.
- https://github.com/AceSLS/SLSsteam — Linux client mod, AGPL-3.0, 529 stars; wiki installation shows `LD_AUDIT` injection.
- https://github.com/AceSLS/SLSsteam/wiki/Installation — install paths, LD_AUDIT line, SteamOS warning.
- https://github.com/ciscosweater/enter-the-wired — ACCELA + SLSsteam installer, MIT, 374 stars.
- https://github.com/SteamClientHomebrew/Millennium — plugin framework, MIT, 4614 stars.
- https://www.steamtools.app/en-EN — SteamTools builds (Windows 2.86, Linux 2.85), fixes, ManiLua providers.
- https://manilua.steamtools.app/premium — premium tier, limits, API access.
- https://github.com/OpenSteam001/OpenSteamTool/releases — Denuvo compatibility, auth sharing, DLL injection, config/lua path, GPL-3.0.
- https://github.com/Yong0512/OpenSteamToolDesktop — Windows desktop wrapper, 29 stars.
- https://github.com/Midrags/SFF — SteaMidra feature list, LumaCore, online-fix search, DLC unlockers, Linux support, 2024 stars.
- https://github.com/SteamAutoCracks/ManifestHub — manifest cache API, 24-hour keys, 2252 stars.
- https://github.com/SteamAutoCracks/Steam-auto-crack — Steamless + Goldberg automation, MIT.
- https://github.com/SteamRE/DepotDownloader — SteamKit2 depot downloader, GPL-2.0, 3526 stars.
- https://github.com/SteamAutoCracks/DepotDownloaderMod — CDN downloader fork used by LuaTools.
- https://generator.ryuu.lol/api — Ryuu API: download by appid, `X-Auth-Key`, lua/manifest, branches.
- https://aitronz.github.io/ost-docs/user/lua-sources — Hubcap ~25/day, Ryuu ~50/day, `.lua` shape, Denuvo `88500005`.
- https://wiki.lua.tools/docs/luatools/guides/hubcapapi — Hubcap key flow, 7-day validity, Luie/Ryuu without keys.
- https://www.sansui233.com/posts/2025-04-22-techs-about-steam-crack — ownership/content/runtime gates, `config.vdf` plaintext key, SteamStub vs Steam API vs Goldberg.
- https://tools.malwaretips.com/url-scan/lua.tools and https://any.run/report/fe1eb6f9eed30d64ab24a59b7bc8491fe9dacdd570b1e67cb6b61596032d99bd/39aab5a1-5a58-45f6-b0e8-df80976444c5 — ecosystem risk signals.
- https://github.com/anticitizn/creamlinux — CreamAPI-like DLC unlocker for Linux, stale since 2022.
- https://github.com/Hintay/vapor-forge — our engine, AGPL-3.0, 4 stars, pushed 2026-11-06.