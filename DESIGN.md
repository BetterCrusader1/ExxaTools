---
name: Native Shelf
colors:
  source: "Adwaita named colors; no bespoke brand palette"
  surfaces: "@window_bg_color, @sidebar_bg_color, @card_bg_color"
  accent: "@accent_bg_color and @accent_color"
  state: "@success_color, @warning_color, @error_color"
typography:
  ui: "Adwaita Sans through GTK/libadwaita"
  data: "monospace, only for identifiers"
  hierarchy: "title-1, title-3, heading, body, caption, dim-label"
spacing:
  unit: "4px"
  rhythm: "4 / 8 / 12 / 16 / 20 / 24 / 32px"
rounded:
  rows: "6px"
  artwork: "8px"
  hero: "12px"
components:
  header: "native adw::HeaderBar with search, refresh and theme"
  navigation: "native ListBox; subtle selected fill plus a 3px left accent"
  library: "artwork-led shelf; no status-card dashboard"
  inspector: "one cinematic game hero and one primary action"
  discover: "uncarded artwork, title and a compact flat Add action"
  details: "depots and plan metadata collapsed until requested"
---

## Overview

ExxaTools is a native Steam companion for a personal game library. Its main job is to let someone find a game, see its state, and launch, add, sync or repair it. It is not a server console and not a web dashboard transplanted into GTK.

The visual direction is **Native Shelf**. GTK and libadwaita own the palette, typography, controls, focus behavior and accessibility semantics. Identity comes from the existing ExxaTools mark, real Steam artwork, the selected game's cinematic hero, and restrained information architecture. A custom color theme is not the product's identity.

The rejected Obsidian Forge direction—near-black violet surfaces, Inter/Geist stacks, status cards and repeated violet accents—must not be reintroduced into the native app. The separate Steam-side panel may retain its own Steam-specific treatment; this document governs the native dashboard.

## Signature Element

The selected game's artwork is the one expressive surface. A 260 px hero uses cover art, a dark scrim for legibility, the game title, state and one primary action. Everything around it stays quiet. If artwork is unavailable, a neutral monogram stands in; it is not recolored into a brand illustration.

The tracked `web/logo.png`, `web/logo-128.png` and `web/logo-48.png` remain the owner's identity. Do not redraw, replace or recolor them as part of UI work.

## Color

Use Adwaita named colors only:

- window and shelf: `@window_bg_color`, `@window_fg_color`
- navigation: `@sidebar_bg_color`, `@sidebar_fg_color`
- raised details: `@card_bg_color`, `@card_fg_color`
- selection and focus: `@accent_bg_color`, `@accent_color`
- semantic state: `@success_color`, `@warning_color`, `@error_color`
- muted text and structure: `@insensitive_fg_color`, `@borders`

Dark and light schemes share component rules. `adw::StyleManager` selects the scheme; CSS does not redefine the palette. Hero text is white only because it sits on artwork and a dark scrim. Status colors communicate state, not decoration.

## Typography

Use the platform font and Adwaita hierarchy classes: `title-1` for page and hero titles, `title-3` for the wordmark and shelf title, `heading` for compact section headings, and `caption` plus `dim-label` for supporting text. Do not load Inter, Geist or a second display family.

Use `monospace` only for app IDs, depot/manifest IDs, paths and similar identifiers. Copy is sentence case, left aligned and written in task language. Avoid decorative eyebrows, all-caps labels and middle-dot metadata chains.

## Layout

```text
┌──────────────────────────────────────────────────────────────────────────┐
│ [ Search Steam by name or app id…          ] [Refresh] [Theme] [— □ ×] │
├──────────────┬───────────────────┬───────────────────────────────────────┤
│ [mark]       │ Library           │ Game artwork / selected page          │
│ ExxaTools    │ summary sentence  │                                       │
│              │ [cover] Game name │ Primary action                        │
│ ▍ Library    │         state     │                                       │
│   Discover   │ [cover] Game name │ [Depots and manifests ▾]              │
│   System     │         state     │ [Plan details ▾]                      │
│   Settings   │                   │                                       │
└──────────────┴───────────────────┴───────────────────────────────────────┘
```

The global header is shared by every page. The navigation rail is 200 px and contains only brand and destinations; engine/version furniture belongs in System, not the rail. The library shelf is about 310–320 px. Content is left aligned and capped with `adw::Clamp` where line length matters.

Library leads with games, not counts or status cards. Discover leads with popular artwork, not a grid of identical bordered cards. System and Settings use native preferences groups. Technical detail is progressive: depot tables and plan metadata stay collapsed until asked for.

## Components

- **Navigation:** native rows with symbolic icons. Selection is a subtle accent fill plus a 3 px left accent; keyboard focus remains a separate visible ring.
- **Library rows:** small cover, game name, one-line state/branch metadata and a semantic status dot. No stat card above the list.
- **Inspector:** one suggested primary action—Launch, Sync manifests or Add to library. Sync/Fix and overflow actions are secondary. Do not give every surface a saturated button.
- **Discover cards:** transparent container, rounded artwork, title and a compact flat `Add` action. An added game shows disabled `Added`; cards do not each get a suggested-action block.
- **Filters:** native linked toggle buttons, not a custom tinted pill bar.
- **Details:** `Depots and manifests` and `Plan details` are collapsed by default. Copy actions remain available for identifiers.
- **System:** native rows and groups explain checks in plain language. `Restart Steam` lives in the Steam UI, not in the native app header or System page.

## Motion and Interaction

Keep the existing short page crossfade and native control feedback. Motion answers navigation or state changes; no entrance choreography, pulsing dots, animated logo or per-card hover theatrics. Keyboard activation, search Enter/clear, row selection and worker commands remain unchanged.

## Do's and Don'ts

**Do** use native Adwaita controls and tokens; let artwork carry color; keep one primary action per detail view; preserve visible focus; keep technical detail one layer down; retain the tracked logo assets.

**Don't** restore a bespoke violet palette or Inter/Geist stacks; add status-card dashboards; use a wall of identical rounded cards; put suggested-action buttons on every Discover card; expose engine/version furniture in the navigation rail; move `Restart Steam` back into the native app; claim rendered visual review without an actual capture.

## Accessibility and Accepted Debt

Native GTK focus order, activation, minimum targets and selection semantics are retained. Color is paired with text, icons or native selection. Reduced-motion behavior remains delegated to the toolkit; custom motion is nonessential.

The fixed rail and shelf can still constrain narrow windows. This direction does not claim a screen-reader audit, WCAG conformance, or a successful rendered screenshot review. Those require separate evidence.

## Change Log

- Removed Obsidian Forge color definitions and Inter/Geist/JetBrains stacks from the native app.
- Replaced custom title sizing with Adwaita hierarchy classes.
- Removed the library status card and navigation engine/version footer.
- Simplified Discover to artwork, title and flat Add actions with native linked filters.
- Moved depot/manifest detail behind a collapsed expander.
- Removed the remaining in-app Restart Steam buttons; Steam owns that action.
