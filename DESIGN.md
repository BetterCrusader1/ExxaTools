---
name: Midnight Cinema
colors:
  source: "libadwaita named colors redefined per theme in src/ui/theme.rs"
  dark: "#0d0e12 window, #111217 sidebar, white 4.5% cards, white 7% borders"
  light: "#f5f6f8 window, #eceef2 sidebar, white cards, black 8% borders"
  accent: "system accent; the selected game's accent inside .game-tint"
  state: "@success_color, @warning_color, @error_color"
typography:
  ui: "platform font through GTK/libadwaita"
  display: "title-1 28px/800, hero title 32px/800, tight letter-spacing"
  data: "monospace, only for identifiers"
spacing:
  unit: "4px"
  rhythm: "4 / 8 / 12 / 16 / 20 / 24 / 32px"
rounded:
  rows: "10-12px"
  posters: "12px"
  hero: "16px"
  actions: "999px (pills)"
components:
  header: "borderless adw::HeaderBar with a pill search, refresh and theme"
  navigation: "native ListBox; soft filled selection, accent icon"
  library: "shelf list + full-bleed hero with the game's logo over a window-wide blurred backdrop"
  discover: "featured carousel, then a grid of 2:3 Steam posters with overlay Add buttons"
  details: "depots and plan metadata collapsed until requested"
---

## Overview

ExxaTools is a native Steam companion for a personal game library: find a game, see its state, and launch, add, sync or repair it. The visual direction is **Midnight Cinema**: a deep, quiet chrome where the game's own art carries the color.

GTK and libadwaita still own controls, focus behavior and accessibility semantics. The look comes from a redefined neutral palette, real Steam artwork (hero, logo, portrait poster) and a small set of component treatments.

The tracked `web/logo.png`, `web/logo-128.png` and `web/logo-48.png` remain the owner's identity. Do not redraw, replace or recolor them as part of UI work.

## Signature Elements

- **Game backdrop.** On Library, the selected game's hero is blurred on the CPU at 160 px (`art::backdrop`) and laid under the whole window with a theme veil. Rail, shelf and header turn translucent over it (`.has-backdrop`), reading as frosted glass without a per-frame GPU blur. It crossfades in 450 ms when the selection changes; System, Settings and Discover stay plain.
- **Game accent.** The same pixels give the art's dominant vivid hue, darkened until white text reaches 4.5:1. It overrides `--accent-bg-color` only inside the inspector (`.game-tint`); gray art keeps the system accent.
- **Hero.** Full pane width, 340 px, Steam `library_hero` art with a bottom-left scrim, Steam `logo.png` trimmed to its visible pixels (the name is its alternative text and the fallback), state pills and one pill primary action.
- **Featured banner.** Discover's Popular view opens with five titles that have hero art; it advances every 7 s, pauses under pointer or focus, and stays still when GTK animations are off.

## Color

Each theme redefines libadwaita's named colors (`window_bg_color`, `sidebar_bg_color`, `card_bg_color`, `borders`, ...) plus `surface_raise`, `surface_hover` and the backdrop veil (`veil_top`, `veil_mid`, `veil_bottom`). The accent is never redefined globally; only `.game-tint` scopes a per-game accent. Text on artwork is white over a dark scrim in both themes. Status colors communicate state, not decoration.

## Typography

Platform font with Adwaita classes. `title-1` is 28 px/800 with -0.5 px tracking; the hero text title is 32 px/800. `heading`, `caption` and `dim-label` keep their roles. `monospace` is only for app IDs, depot/manifest IDs and paths. Copy is sentence case.

## Layout

```text
┌──────────────────────────────────────────────────────────────────────────┐
│ (  Search Steam by name or app id…           ) (⟳) (☾)        — □ ×   │
├──────────────┬───────────────────┬───────────────────────────────────────┤
│ [mark]       │ Library           │ ┌───────────────────────────────────┐ │
│ ExxaTools    │ summary           │ │ Ready  2 DLC                      │ │
│              │ [cov] Game name • │ │ [ GAME LOGO ]        hero art     │ │
│ ▓ Library    │ [cov] Game name • │ │ meta                              │ │
│   Discover   │ [cov] Game name • │ │ (Launch) (Sync) (Fix)         (⋮) │ │
│   System     │                   │ └───────────────────────────────────┘ │
│   Settings   │   blurred game backdrop under everything                  │
└──────────────┴───────────────────┴───────────────────────────────────────┘
```

The navigation rail is 200 px (64 px icon-only on narrow windows) and holds only brand and destinations. The shelf is about 310–320 px. The hero and the details below share the inspector's left edge.

## Components

- **Buttons:** 10 px radius by default. `suggested-action` is a pill with a soft glow in the current accent. Hero secondaries are glass pills (white 13%, no border).
- **Filters:** `.segmented` pill control, the checked segment raised on `card_bg_color`.
- **Library rows:** 64×30 cover, name, one-line meta, status dot; soft filled selection.
- **Discover cards:** 180×270 poster (`library_600x900`, or the store API's hashed `library_capsule_2x`), hover zoom inside the frame, title below, circular overlay Add button that turns accent on hover and green when added. A header-only title is contained over its own blurred art.
- **Art loading:** every image goes through `src/ui/art.rs`: one decoder thread, a 48 MB LRU, a skeleton while loading and a 220 ms fade on arrival.

## Motion

Page crossfade 180 ms, backdrop crossfade 450 ms, art fade 220 ms, hover transitions 140–260 ms, featured auto-advance every 7 s. All CSS transitions follow GTK's animation setting; the carousel checks it explicitly.

## Do's and Don'ts

**Do** let artwork carry color; keep one primary action per detail view; keep technical detail one layer down; route all images through `art.rs`; keep the accent scoped.

**Don't** redefine the accent globally; add per-frame GPU blur; reintroduce status-card dashboards; put `Restart Steam` back into the native app; claim rendered review without an actual capture.

## Accessibility and Accepted Debt

Native focus order, activation and selection semantics are retained. Game accents are contrast-guarded for white text (4.5:1); the logo keeps the game name as alternative text. Text over bright art relies on the veil and hero scrim.

Adaptive layout: minimum 420 px; below 1000 px the rail is icon-only, below 760 px the shelf is an overlay (`adw::OverlaySplitView`) and the hero's secondary actions move into its overflow menu. Open: no screen-reader audit or WCAG conformance claim; Discover's featured heroes cost about 20 MB more than the previous header-only grid.
