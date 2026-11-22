# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Linux players who use the Steam client and want games in it that Steam itself will not set up for them. ExxaTools is a public product (MIT, packaged for Arch via `packaging/PKGBUILD`). The most frequent job is adding a new game: find it in Discover or search, add it, and let Steam download it like any other title. Launching, syncing manifests and fixing update loops follow from that.

## Product Purpose

Pick a game, pin its depots and manifests, store depot keys, and Steam downloads it through the real CDN as if it were owned. Success is a game that appears in the Steam library, downloads, launches and stays on the chosen build without the user touching Steam's files or a terminal.

## Positioning

ExxaTools lives inside the Linux Steam client rather than beside it: it starts with Steam through the container launcher wrapper, injects its panel and store-page actions into Steam's own UI over the CEF debugging port, and opens its dashboard when Steam does. It drives Vapor Forge (an LD_AUDIT shim) and keeps the engine patch for current Steam builds in this repository.

## Operating Context

- Starting Steam starts ExxaTools: the API server on `127.0.0.1:47865`, the panel next to Steam's Help menu, a card on every store page, and the native dashboard window.
- Plans land in `~/.config/vapor-forge/scripts/<appid>.lua`; manifests go to Steam's `depotcache`.
- Depot keys come from Hubcap, from a key pasted by hand, or from a custom `{depotId}` source; request codes come from an ordered list of public and personal mirrors.
- A command line (`exxa search`, `exxa plan`, `exxa fix`, `exxa doctor`, ...) mirrors the dashboard for scripting and troubleshooting.

## Capabilities and Constraints

- The dashboard is a native GTK4 / libadwaita desktop app in Rust (Linux only). `Platform: web` is the closest value the schema offers; desktop conventions and GNOME/libadwaita behavior apply, not browser ones.
- Dashboard areas: Library (shelf, per-game hero, depots and manifests, plan details), Discover (featured titles and popular posters from the Steam store), System (doctor checks with fixes), Settings (sources, keys, request-code mirrors, Steam coupling).
- Interface language: English.
- Steam artwork (headers, portrait posters, library heroes, logos) comes from Steam's CDN and store API and is cached locally.
- Steam will not hand out depot keys or request codes for unowned apps; those always come from external sources that can be slow, rate limited or down.

## Brand Commitments

- Name: ExxaTools.
- The mark in `web/logo.png`, `web/logo-128.png` and `web/logo-48.png` is the owner's identity and must not be redrawn, replaced or recolored.

## Evidence on Hand

- Real Steam artwork for any app, fetched at runtime.
- `assets/dashboard.png`, a screenshot of the current dashboard.
- No user testimonials, usage numbers, press or benchmarks exist; do not invent them.

## Product Principles

1. Adding a game is the main path; every screen should make the next add or launch obvious.
2. Feel like part of Steam: no terminal, no manual file edits, no separate ritual to start it.
3. Technical detail (depots, manifest ids, plan metadata) stays one layer down until asked for.
4. Failures name the broken link and its fix rather than a generic error.
