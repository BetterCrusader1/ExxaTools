# ExxaTools — design

The dashboard is a native GTK4 + libadwaita **depot desk**. It is one window
with three working panes, not a web page in a frame.

## The idea

The library is the object a person manages. The app opens on it, keeps the
selected game visible, and puts everything needed to launch or repair that game
in one inspector. Steam's own client is where the game lives; ExxaTools is the
desk beside it.

Information architecture:

```
┌ rail ─────┬ shelf ──────────┬ inspector ─────────────────────┐
│ brand     │ Library     12  │ hero artwork                    │
│ Library   │ search result   │ Project Zomboid                 │
│ Discover  │ rows with art   │ ready · 108600 · 9 depots       │
│ System    │                 │ [Launch] [Sync] [⋯]             │
│ Settings  │                 │ Depots table                    │
│           │                 │ Plan details                    │
│ Refresh   │                 │                                 │
│ Steam ●   │                 │                                 │
└───────────┴─────────────────┴─────────────────────────────────┘
```

Discover, System and Settings replace the whole content area. They are not
modal dialogs and nothing is drawn on top of another application.

## Themes: Nacre

Two themes share the same structure. The toggle lives in the header and the
choice is stored with the rest of the settings.

**Nacre dark** is the default: pure black base, near-black violet surfaces,
pearl highlights and a deep-violet primary action. **Nacre light** is a lilac
pearl: a lavender canvas, white glass surfaces, deep violet text and the same
pearl CTA.

| Token | Dark | Light | Role |
| --- | --- | --- | --- |
| canvas | `#000000` | `#F0E5FF` | window base |
| rail | `#0A0612` → `#030106` | `#FFFFFF` → `#EADDFF` | navigation gradient |
| surface | `#0B0612` | `#FFFFFF` | cards and lists |
| surface raised | `#130A1E` | `#FBF7FF` | table and summary gradients |
| text | `#ECE8F6` | `#1A0B2E` | primary copy |
| muted | `#C4B7DB` | `#3F3354` | metadata and hints |
| accent | `#7C5CFF` | `#6D28D9` | selection and focus |
| pearl A/B/C | `#F1E8FF` / `#C9B6FF` / `#8B5CF6` | `#FFFFFF` / `#D9C7FF` / `#A78BFA` | CTA sheen |
| success | `#7EE0C3` | `#2F8F6B` | ready plans, loaded keys |
| warning | `#F2C78B` | `#B7791F` | setup needed |
| danger | `#FF8AA8` | `#C2415A` | failed checks |

Every surface carries a thin border and a top inner highlight; the pearl only
gets spent on the primary action, the selected row and the hero. Type is Noto
Sans throughout — no display face, no monospace chrome. Headings use 700–800
weight; technical identifiers stay in the same family and rely on alignment,
not a second typeface.

## Motion

The stack crossfades at 220 ms. Rows, cards and buttons transition background,
border and glow at 160–220 ms. The ready dot gets a slow breathing halo. Motion
is limited to state changes; there is no entrance animation on every section.
Reduced-motion support stays on the list when the animation set is imported.

## Components

- **Navigation rail**: brand mark, four destinations, a global refresh action,
  and the Steam connection state. It never collapses into a floating window.
- **Shelf**: one row per game, art at 64×30, full name, branch and depot count,
  and a status dot. Selection is a tinted row, not a card.
- **Inspector**: a hero panel with the game art, a dark glass title panel, one
  pearl primary action, a tonal sync button and an overflow menu. Depots are a
  compact aligned table; plan data stays behind one expander.
- **Discover**: a four-column cover grid with a consistent card size and a
  monogram fallback when Steam has no header art.
- **System**: severity-first summary; warnings are lifted into **Needs
  attention** before the healthy groups. Technical hints appear only on a
  check that needs action.
- **Settings**: Adwaita preference groups, masked key rows, one save action and
  a toast confirmation.

## States

Every page has an explicit loading, empty and error state. A library with no
games says what to do next. A search with no matches says so without hiding the
query. A discover request that fails offers a retry. Selecting nothing shows a
quiet status page with one route to Discover. Long names ellipsize; rows keep
their height; buttons keep their width.

## Accessibility and keyboard

GTK focus is visible on every control. ListBox rows are reachable with arrow
keys, search is a real SearchEntry, dialogs trap focus and return it. Status is
never carried by colour alone: ready, missing keys and failure also have words.
Touch targets are at least 30px. The window has a minimum useful width and the
three panes keep their scroll areas independent.

## Implementation

`src/ui` is deliberately small modules rather than one window file:

| Module | Responsibility |
| --- | --- |
| `model.rs` | pure view models and formatting, unit-tested |
| `covers.rs` | Steam header art cache |
| `theme.rs` | Nacre dark/light palettes, CSS and the theme switch |
| `worker.rs` | commands and events; no network on the GTK thread |
| `shell.rs` | window, rail, header, stack, event loop |
| `shelf.rs` | library row builder and selection |
| `inspector.rs` | selected-game surface |
| `discover.rs` | cover grid |
| `system.rs` | health report |
| `settings.rs` | preferences |

`src/window.rs` is only a compatibility launcher for `crate::ui::run`.

The window also accepts `?page=discover`, `?page=system` or `?page=settings`
through its URL argument, so a store-page card or a script can open the right
view without simulating clicks.