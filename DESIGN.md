---
name: Obsidian Forge
colors:
  primary: "#8B5CF6"
  on-primary: "#09090D"
  primary-hover: "#A78BFA"
  primary-active: "#7C3AED"
  on-primary-active: "#FFFFFF"
  surface: "#09090D"
  on-surface: "#F4F2FA"
  surface-sidebar: "#0E0E14"
  on-surface-sidebar: "#F4F2FA"
  surface-raised: "#14141D"
  on-surface-raised: "#F4F2FA"
  text-secondary: "#AAA5B8"
  text-muted: "#858092"
  border: "#2A2935"
  border-strong: "#3A3748"
  ring: "#C4B5FD"
  success: "#10B981"
  on-success: "#09090D"
  warning: "#F59E0B"
  on-warning: "#09090D"
  error: "#EF4444"
  on-error: "#09090D"
typography:
  ui: "Inter, Geist, Noto Sans, Adwaita Sans, sans-serif"
  data: "JetBrains Mono, Geist Mono, Noto Sans Mono, Adwaita Mono, monospace"
  caption: "11px/1.45 500"
  body: "13px/1.5 400"
  label: "13px/1.4 600"
  section: "15px/1.35 650"
  title: "26px/1.2 700"
  display: "28px/1.15 750"
spacing:
  unit: "4px"
  xxs: "4px"
  xs: "8px"
  sm: "12px"
  md: "16px"
  lg: "20px"
  xl: "24px"
  xxl: "32px"
rounded:
  radius: "8px"
  small: "6px"
  medium: "8px"
  large: "12px"
  hero: "16px"
  pill: "999px"
components:
  window: "background {colors.surface}; color {colors.on-surface}; font {typography.ui}"
  sidebar: "background {colors.surface-sidebar}; border-right 1px {colors.border}"
  header: "background {colors.surface-sidebar}; border-bottom 1px {colors.border}"
  card: "background {colors.surface-raised}; color {colors.on-surface-raised}; border 1px {colors.border}"
  button-primary: "background {colors.primary}; color {colors.on-primary}; radius {rounded.medium}"
  button-primary-hover: "background {colors.primary-hover}; color {colors.on-primary}"
  button-primary-active: "background {colors.primary-active}; color {colors.on-primary-active}"
  button-focus-visible: "outline 2px {colors.ring}; outline-offset 2px"
  input: "background {colors.surface-raised}; color {colors.on-surface-raised}; border 1px {colors.border-strong}"
  input-focus: "border 1px {colors.ring}; outline 2px {colors.ring}"
  nav-selected: "background color-mix({colors.primary} 7%, transparent); inset 3px {colors.primary}"
  nav-focus-visible: "inset 1px ring at 80% opacity; distinct from selection"
  discover-action: "compact 30px structural footer button; transparent surface; 1px restrained violet border"
  discover-action-hover: "10% violet wash; stronger violet border"
  discover-action-active: "20% violet wash; solid violet border"
  status-success: "color {colors.success}; background color-mix({colors.success} 13%, transparent)"
  status-warning: "color {colors.warning}; background color-mix({colors.warning} 13%, transparent)"
  status-error: "color {colors.error}; background color-mix({colors.error} 13%, transparent)"
---

## Overview

Obsidian Forge is the native shell for people maintaining a Steam library through ExxaTools. It must feel **precise**, **composed**, and **forged**: a dependable instrument for repeated maintenance, not a gaming storefront or a neon control panel.

The primary taste direction is **Operational**. Native GTK and libadwaita controls retain their behavior, focus handling, window controls, and accessibility semantics. Identity comes from the three-step obsidian surface hierarchy, the disciplined violet selection/action language, and the existing ExxaTools mark. Game artwork supplies broad color; the shell does not compete with it. The light theme is a functional alternative, not a second brand.

The model-default editorial direction—cream, serif display type, terracotta, and wide reading margins—is explicitly rejected because this is a data-dense developer utility. No external design or image service is used.

## Signature Element

The existing ExxaTools mark is retained as the sole decorative signature; the rejected replacement forge/E artwork is not part of the design. The tracked `web/logo.png`, `web/logo-128.png`, and `web/logo-48.png` assets supply the navigation brand and desktop icon. Their established silhouette and colors remain unchanged, including when the 48 px asset is rendered at 34 px in the rail.

The second identifying device is functional rather than decorative: the selected navigation row carries a 3 px violet **left accent** over a quiet surface, like a tool seated in a forge slot. Keyboard focus remains a separate, visible inset ring. No other navigation ornament is allowed.

## Colors

The default dark theme uses the exact pinned foundation tokens:

| Role | Token | Value | Use |
|---|---|---:|---|
| Window | `surface` | `#09090D` | Main canvas and deepest recess |
| Sidebar/header | `surface-sidebar` | `#0E0E14` | Persistent chrome |
| Raised work surface | `surface-raised` | `#14141D` | Inputs, cards, tables, popovers |
| Primary action | `primary` | `#8B5CF6` | Primary buttons, active navigation accent |
| Pressed action | `primary-active` | `#7C3AED` | Pressed primary action |
| Success | `success` | `#10B981` | Ready/healthy state only |
| Warning | `warning` | `#F59E0B` | Setup needed/attention only |
| Error | `error` | `#EF4444` | Failure/destructive state only |

Primary text is `#F4F2FA`, secondary text `#AAA5B8`, muted text `#858092`, borders `#2A2935` and `#3A3748`, and the independent focus ring `#C4B5FD`. Hover violet is `#A78BFA`. Surfaces and text are paired by token; components do not improvise combinations.

The intended proportion is roughly **70% window canvas, 20% sidebar/header, 8% raised surfaces, and at most 2% violet**. Semantic hues are excluded from the brand accent budget but remain state-only. No cyan-to-purple gradients, glass blur, ambient glow, or purple tint is added to every surface.

Measured normal-text contrast ratios include primary text on the window at **17.92:1**, secondary at **8.31:1**, muted at **5.21:1**, dark text on primary violet at **4.69:1**, and white on pressed violet at **5.70:1**. Success, warning, and error on the window measure **7.84:1**, **9.26:1**, and **5.28:1**. The independent ring on the window is **10.77:1**. These are token-pair calculations, not a rendered accessibility audit.

The light theme redefines the same semantic tokens rather than branching component rules:

| Token group | Light values |
|---|---|
| Surfaces | `surface #F6F5F8`, `surface-sidebar #F0EDF4`, `surface-raised #FFFFFF` |
| Text | `on-surface #211D29`, `text-secondary #625B6D`, `text-muted #71697E` |
| Brand/action | `primary #7C3AED`, `on-primary #FFFFFF`, `primary-hover #8B5CF6`, `primary-active #6D28D9` |
| Structure/focus | `border #DCD7E3`, `border-strong #C9C2D2`, `ring #6D28D9` |
| State | `success #087A55`, `warning #8A5300`, `error #C81E3A` |

## Typography

There are two functional stacks, not two display personalities:

- **UI:** `"Inter", "Geist", "Noto Sans", "Adwaita Sans", sans-serif`. Inter is the first choice; Geist is the sanctioned alternate. Noto Sans and Adwaita Sans are installed local fallbacks. Use 400 for body, 500 for quiet metadata, 600 for labels/navigation, and 650–750 for section/page hierarchy.
- **Data:** `"JetBrains Mono", "Geist Mono", "Noto Sans Mono", "Adwaita Mono", monospace`. JetBrains Mono is first choice; Geist Mono is the alternate. Noto Sans Mono and Adwaita Mono are installed local fallbacks. Use only for app IDs, versions, paths, depot keys, command output, and compact engine metadata—not navigation or prose.

The scale is 11 px caption, 12 px dense metadata, 13 px body/navigation, 15 px section heading, 20 px wordmark, 26 px page title, and 28 px inspector/display title. Body line-height is 1.5; headings use 1.15–1.35; captions never drop below 1.4. Use sentence case, not tracked all-caps eyebrows. Weight and size establish hierarchy; no second serif/display family is introduced.

CJK inherits the Noto Sans system fallback. Keep CJK body at 14 px or larger with 1.55 line-height and zero added letter spacing. Never force CJK labels into the data stack. Ellipsize only trailing identifiers where the full value remains available by tooltip or copy action.

## Layout

The base unit is **4 px**, with a working rhythm of **4 / 8 / 12 / 16 / 20 / 24 / 32 px**. Compact controls use 8–12 px internal spacing; page and section separation uses 20–32 px. Alignment is left-led and scan-friendly. Stable dimensions outrank decorative centering.

```text
┌──────────────────────────────────────────────────────────────────────────┐
│ [ Search Steam by name or app id…          ] [Refresh] [Theme] [— □ ×] │
├──────────────┬───────────────────┬───────────────────────────────────────┤
│ [mark]       │ Library     count │ Page title                            │
│ ExxaTools    │                   │                                       │
│              │ [cover] Game name │ Content owned by the selected page    │
│ ▍ Library    │         Ready     │                                       │
│   Discover   │ [cover] Game name │ Primary action / details              │
│   System     │         Setup     │                                       │
│   Settings   │                   │                                       │
│              │                   │                                       │
│ Vapor Forge  │                   │                                       │
│ ExxaTools v… │                   │                                       │
└──────────────┴───────────────────┴───────────────────────────────────────┘
```

The **single global header** sits above the page stack. Search, refresh, theme, and native window controls therefore keep identical position, size, icon, and behavior on Library, Discover, System, and Settings. No page may recreate them. Refresh owns the existing library/doctor refresh plus manifest sync; the sidebar has no duplicate action.

The navigation rail is **200 px**. The library shelf begins at **320 px** and the inspector owns the remaining width. The page stack owns page changes; each view's existing scroller owns vertical scrolling. The shell does not add another scroll container. Existing paned behavior and page bindings remain intact. Narrow-window behavior is constrained by the current GTK layout and is recorded as accepted debt below rather than silently claiming responsive behavior.

## Elevation & Depth

There is no shadow-based elevation system. Depth comes from the three surface tokens, 1 px structural borders, and native popover/dialog treatment. `box-shadow` is reserved for the 3 px selected-row inset accent and never used as a blanket card shadow. No backdrop blur, glass, ambient bloom, or gradient panel decoration.

## Shapes

One base radius knob is **8 px**. Small artwork and dense rows use 6 px; controls use 8 px; navigation rows use 6 px; cards/tables use 12 px; the existing artwork hero may use 16 px. Pills alone use 999 px. The geometry is quiet and consistent rather than uniformly rounded cards everywhere.

## Components

- **Global header:** native `adw::HeaderBar`; one centered search entry, refresh and theme icon buttons packed at the end, and native window controls. Header height and control order remain stable across pages.
- **Search:** raised surface, 1 px strong border, 8 px radius, visible 2 px focus ring. Placeholder uses secondary/muted text. Activation and clear behavior retain existing `Command::Search` and `Command::LoadLibrary` bindings.
- **Refresh/theme:** native flat icon buttons with existing symbolic icons and pointer cursor. Refresh retains library and doctor loads and now also carries the moved `Command::SyncAll` binding. Theme retains `Command::SetTheme`, icon updates, and persisted worker events.
- **Navigation:** native `ListBox` rows with 16 px symbolic icons and 13 px labels. Default is quiet; hover uses a subtle violet wash; selected uses a 7% dark-theme (8% light-theme) violet wash plus a 3 px left accent; focus-visible uses a separate 1 px inset ring at 80% opacity so selection never becomes a bright outer box. Disabled/loading states remain native GTK states and do not alter layout.
- **Brand/footer:** the existing ExxaTools mark is retained and rendered at 34 px beside the 20 px wordmark. At the bottom, compact two-line text reads `Vapor Forge engine` and `ExxaTools v0.3.0` (version interpolated from Cargo). No footer button.
- **Buttons:** suggested actions use primary/on-primary; hover and pressed use their sibling tokens; flat actions remain quiet until hover. Discover's repeated card action is instead a compact 30 px structural footer button: transparent by default, with a restrained violet border/text accent, a darker separated footer, and explicit hover/focus/pressed washes. Existing destructive/loading labels and commands are not changed by the shell.
- **Inputs/cards/tables:** native entries and lists on raised surfaces, 1 px borders, no decorative shadow. Discover cards retain a clearly separated raised surface and an inner cover frame; row hover is subtle; selection never relies on color alone where a left accent or native selected state is available.
- **Status:** pills and dots use success/warning/error only for their semantic states. Neutral counts use violet. Empty/loading/error content remains owned by each existing view; SC1 does not invent replacement states or alter view logic.
- **Logo:** the existing tracked PNGs are the source of truth: 512×512, 128×128, and 48×48. No replacement SVG is generated or referenced.

## Motion & Interaction

Use **fast 120 ms linear** for pressed feedback, **standard 160 ms ease-out** for hover/color/border changes, and **page 180 ms ease-in-out** for the existing crossfade. Animate only state changes that answer user input: page selection, hover, focus, and pressed state. Do not animate the logo, navigation accent independently, status dots, window entry, or every card.

GTK's reduced-motion preference should suppress nonessential transitions; where GTK does not expose a CSS media query for it, keep transitions short, nonspatial, and nonessential. Never bounce, overshoot, autoplay, or use motion to communicate an operation without text/native progress feedback. Keyboard activation, search Enter/clear behavior, row selection, and all worker commands remain unchanged except for the deliberate consolidation of manifest sync into global refresh.

## Do's and Don'ts

**Do** keep one header and one navigation rail; use native controls; align to the 4 px rhythm; keep artwork as the main color source; use violet for selection/action; preserve visible focus; keep the engine/version footer quiet; retain the exact tracked ExxaTools PNG assets.

**Don't** duplicate refresh in the sidebar; add page-local window controls; put gradients/glows behind every panel; use all-caps eyebrow labels; turn metadata into prose; add equal card grids as decoration; put mono text everywhere; replace the established brand mark; turn every repeated card action into a saturated primary button; claim a visual pass without captures.

### Self-review against generic dark-purple UI

- **Near-black plus violet is inherently common.** The differentiator is not the hue alone: three exact neutral-cool surface steps, the established ExxaTools mark, and a functional left-slot selection accent form a product-specific system. There is no purple gradient or glow.
- **Inter can read as a default.** It is retained because the brief explicitly selects it and because operational legibility matters; the existing brand mark, weight scale, and compact engine footer carry identity instead of forcing a display face onto a tool.
- **Dark cards and status pills can become SaaS boilerplate.** Cards remain structural bordered surfaces, status colors remain semantic, and no new card layout or marketing hero is introduced.
- **Monospace metadata can become terminal cosplay.** Mono is restricted to identifiers and version data; navigation and explanations stay proportional.
- **Motion can become decorative.** Only brief user-driven transitions remain; there is no entrance choreography or pulsing status.
- **The light theme is not an inverted afterthought.** It redefines semantic tokens with measured text/action contrast while preserving the same component geometry.

The plan therefore passes the textual anti-generic review. Rendered visual review remains a separate evidence requirement.

## Accessibility Constraints & Accepted Debt

Keyboard paths remain those of native GTK: global search, buttons, list rows, native window controls, and page content keep their existing focus order and activation behavior. Buttons and inputs use a 2 px ring with 2 px offset; navigation rows use the documented 1 px inset ring so keyboard focus stays visible without turning selection into a bright outer box. Pointer targets use native GTK minimum sizing; primary icon controls should render at least 32×32 px, exceeding WCAG 2.2's 24×24 minimum. Color is paired with native selection, text, icon, or labels. Nothing in SC1 adds drag-only, timed, or pointer-only interaction.

**Accepted debt:** SC1 is limited to tokens, brand, and shell. It does not prove WCAG conformance, run a screen reader, measure every composed/translucent state, or change narrow-window/reflow behavior in individual views. The existing 200 px rail plus 320 px shelf can constrain compact widths; later view work must address responsive/keyboard-safe layouts. Desktop and compact captures, small-logo inspection, and light/dark rendered contrast review remain unobserved until actually captured.

## Research Log

- **Repository baseline:** read `baseline.md`, `src/ui/theme.rs`, `src/ui/shell.rs`, `Cargo.toml`, and asset references. Took the existing native widget architecture, page/event bindings, 512/128/48 packaging contract, and green baseline as constraints.
- **Local reference data:** ran `omh design data` for dev-tool palette, font, and UX rows. Took the two-step surface separation, Inter/data-mono role split, and explicit error/progress guidance; did not copy the reference palette because Obsidian Forge pins its own exact tokens.
- **Installed font inventory:** queried `fc-list`. Confirmed Noto Sans, Adwaita Sans, Noto Sans Mono, and Adwaita Mono as local fallbacks; Inter, Geist, JetBrains Mono, and Geist Mono were not observed in the installed list.
- **Native design guidance:** consulted the loaded frontend-design references. Retained standard GTK/libadwaita controls and limited identity to tokens and the existing artwork rather than painting a web-style dashboard over native widgets.
- **Rejected visual repair:** restored the tracked ExxaTools PNGs after the replacement mark was rejected. Repeated Discover actions now use compact outlined footer buttons rather than saturated suggested-action blocks; selected navigation uses a quieter left-accent treatment with a separate inset keyboard-focus ring.
- **Contrast checks:** calculated WCAG relative-luminance ratios locally for the named token pairs. These calculations guide token choices and do not substitute for rendered accessibility testing.
- **External visual research:** skipped by scope; no external services were allowed or needed. Existing Steam artwork and native operational tools are the reference direction.
