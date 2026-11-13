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

## Palette and type

Adwaita dark supplies the palette, so the app follows the desktop theme and
accent. Only semantic colours are added:

| Token | Role |
| --- | --- |
| `@window_bg_color` | window and shelf base |
| `@sidebar_bg_color` | navigation rail |
| `@headerbar_bg_color` | title bar chrome |
| `@card_bg_color` | inspector tables and summary |
| `@accent_bg_color` / `@accent_color` | selection, primary action, counts |
| `@success_color` | ready plans, loaded depot keys |
| `@warning_color` | setup needed, missing key |
| `@error_color` | failed check |
| `@insensitive_fg_color` | captions and technical detail |

Type is the system UI face; Noto Sans Mono is used only for app ids, depot ids
and manifest numbers. Headings are 26–28px with weight 700–800; body copy stays
at the system size; captions are 11px. No all-caps labels and no decorative
gradients.

## Components

- **Navigation rail**: brand mark, four destinations, a global refresh action,
  and the Steam connection state. It never collapses into a floating window.
- **Shelf**: one row per game, art at 64×30, full name, branch and depot count,
  and a status dot. Selection is a tinted row, not a card.
- **Inspector**: header art, title, identity line, status pill, one primary
  action, secondary sync, and an overflow menu. Depots are a compact aligned
  table, not repeated list cards. Plan data is collapsed behind one expander.
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