# ExxaTools — design

One screen, one job. The app has three views: **Library**, **Game**, **System**.
No permanent log panels or raw paths on the first screen — those live wherever
people go looking for them.

## Palette

| Token | Hex | Role |
| --- | --- | --- |
| `--ink` | `#0e1013` | window background |
| `--rail` | `#13171c` | left navigation |
| `--surface` | `#181d24` | panels |
| `--raised` | `#1f2630` | buttons, active row |
| `--line` | `#262f3a` | borders |
| `--text` | `#eff3f8` | primary text |
| `--dim` | `#a9b4c3` | secondary text |
| `--faint` | `#7f8b9a` | captions |
| `--sand` | `#edb45f` | accent, primary action |
| `--mint` | `#6fd3a0` | success |
| `--rose` | `#e4796d` | failure |

Gold belongs to the primary action and the selected state. Everything else is a
grey surface. Secondary text keeps AA contrast on `--surface`.

## Typography

Montserrat variable 400–800, shipped locally in `web/fonts/` so the window works
offline. Mono only for manifest ids, depot keys and logs. Scale: 34 / 22 / 16 /
15 / 13 / 11.5. Headings are sentence case, no uppercase tracking.

## Frame

```
┌ rail 224 ┬────────────── stage (max 1220) ──────────────┐
│ brand    │  page head                                     │
│ Library  │  ┌ search 56px ─────────────────────────────┐  │
│ System   │  │                                          │  │
│ …        │  └──────────────────────────────────────────┘  │
│          │  grid of portrait cards (158px, 3:4)           │
└──────────┴────────────────────────────────────────────────┘
```

Game view: a 244px hero with the game's `library_hero.jpg` and a left-to-right
gradient, title at 34px, metadata, then the action row. Install is the only gold
button on the screen. Below it, two columns: **Depots** and **Build**.

## Components

- **Game card**: 3:4 portrait, 244px tall, hover lifts 3px and adds a shadow;
  caption 13px/600 with the app id underneath.
- **Hero**: background `library_hero.jpg`, gradient 94% → 18% horizontally and
  92% vertically, so text never sits on raw art.
- **Depots**: table rows under a `depot / size / key` header. Checkbox 17px,
  size right-aligned with `tabular-nums`, key is a 168px mono field that turns
  green when filled.
- **Build bar**: segments sized by each depot's share of the total; selected
  depots are gold. Caption underneath: `N of M depots · 14.7 GiB`.
- **Buttons**: 38px tall, 10px radius. Primary is gold with dark text; ghost is
  transparent with a hairline border; warn uses gold text and border.
- **Chips**: 30px pill; active state is a gold border over a 12% gold fill.
- **Panels**: `--surface`, 18px radius, 20px padding, 1px `--line-soft` border.
- **Menu**: 38px button with our mark in the top right, 250px panel.

## Motion

One entrance per view change (220ms, fade plus 6px rise). Hover and focus 140ms.
`prefers-reduced-motion` removes all of it.

## Accessibility

Focus is always visible (2px gold outline). Hit targets are at least 30px.
Status is never colour alone — a label sits next to it. Text contrast ≥ 4.5:1.

## Responsive

≥1080 shows two columns. Below 900 the rail becomes a top bar. Below 620 the grid
drops to two columns, the key field takes the full width, and the hero shrinks to
240px.