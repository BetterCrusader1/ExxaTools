//! Steam native styling: the Steam client's own library palette and keys,
//! expressed through libadwaita's named colors and accent variables, so the
//! dashboard reads as part of Steam.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    pub fn from_setting(value: &str) -> Self {
        if value.eq_ignore_ascii_case("light") {
            Self::Light
        } else {
            Self::Dark
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Dark => "weather-clear-night-symbolic",
            Self::Light => "display-brightness-symbolic",
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Dark => "Switch to the light theme",
            Self::Light => "Switch to the dark theme",
        }
    }

    pub fn color_scheme(self) -> adw::ColorScheme {
        match self {
            Self::Dark => adw::ColorScheme::ForceDark,
            Self::Light => adw::ColorScheme::ForceLight,
        }
    }
}

/// Each theme redefines libadwaita's named colors, so every native widget
/// follows; `THEME` holds the shared component rules.
pub fn css(theme: Theme) -> String {
    let palette = match theme {
        Theme::Dark => DARK,
        Theme::Light => LIGHT,
    };
    format!("{palette}{THEME}")
}

const DARK: &str = r#"
@define-color window_bg_color #000000;
@define-color view_bg_color #000000;
@define-color headerbar_bg_color #000000;
@define-color headerbar_backdrop_color #000000;
@define-color sidebar_bg_color #000000;
@define-color sidebar_backdrop_color #000000;
@define-color card_bg_color #0e0d12;
@define-color popover_bg_color #141219;
@define-color dialog_bg_color #0e0d12;
@define-color window_fg_color #ecebf2;
@define-color view_fg_color #ecebf2;
@define-color borders #1f1d27;
@define-color muted_fg #8c8a99;
@define-color surface_raise #0f0e14;
@define-color surface_hover #1a1822;
@define-color pearl #7b5cf0;
@define-color pearl_light #b7a4ff;
@define-color pearl_deep #4a2fb8;
@define-color success_color #8ee6b0;
@define-color warning_color #e6b450;
@define-color error_color #ff8a80;
@define-color card_stock #17151f;
@define-color veil_top rgba(0, 0, 0, 0.35);
@define-color veil_mid rgba(0, 0, 0, 0.74);
@define-color veil_bottom rgba(0, 0, 0, 0.95);
"#;

const LIGHT: &str = r#"
@define-color window_bg_color #e9ecef;
@define-color view_bg_color #e9ecef;
@define-color headerbar_bg_color #dfe3e6;
@define-color headerbar_backdrop_color #dfe3e6;
@define-color sidebar_bg_color #dfe3e6;
@define-color sidebar_backdrop_color #dfe3e6;
@define-color card_bg_color #f6f7f8;
@define-color popover_bg_color #ffffff;
@define-color dialog_bg_color #ffffff;
@define-color window_fg_color #1b1e22;
@define-color view_fg_color #1b1e22;
@define-color borders #c5cbd1;
@define-color muted_fg #5d6670;
@define-color surface_raise #dde1e5;
@define-color surface_hover #cfd5db;
@define-color pearl #6a4ae0;
@define-color pearl_light #8b70f0;
@define-color pearl_deep #4a2fb8;
@define-color success_color #1d7a4a;
@define-color warning_color #8f5d00;
@define-color error_color #b3261e;
@define-color card_stock #d9dde3;
@define-color veil_top rgba(233, 236, 239, 0.72);
@define-color veil_mid rgba(233, 236, 239, 0.90);
@define-color veil_bottom rgba(233, 236, 239, 0.97);
"#;

/// Steam native: the Steam client's own library language, read from its
/// stylesheet (steamui/css/library.css): graphite #1b1e22 to #3d4450
/// surfaces, #dfe3e6 text, a blue #1a9fff accent, the green play gradient
/// and the blue install gradient, 2 px corners. ExxaTools runs inside Steam
/// and should look like part of it; the game's art stays the loud element.
pub const THEME: &str = r#"
:root {
    --accent-bg-color: @pearl;
    --accent-fg-color: #ffffff;
    --success-color: @success_color;
    --warning-color: @warning_color;
    --error-color: @error_color;
}

image.success, label.success { color: @success_color; }
image.warning, label.warning { color: @warning_color; }
image.error, label.error { color: @error_color; }

.title-1 { font-size: 30px; font-weight: 800; letter-spacing: -0.3px; }
.title-3 { font-weight: 800; }
.hero-title { font-size: 36px; font-weight: 800; letter-spacing: -0.5px; }
.heading { font-weight: 700; }
.dim-label, .caption.dim-label { color: @muted_fg; opacity: 1; }

headerbar.global-header {
    box-shadow: none;
    border-bottom: 1px solid rgba(0, 0, 0, 0.45);
    min-height: 48px;
}
entry.global-search, .global-search {
    border-radius: 3px;
    background-color: @card_bg_color;
    box-shadow: inset 0 0 0 1px @borders;
    min-height: 32px;
    padding: 0 12px;
    color: @window_fg_color;
}
.global-search:focus-within {
    box-shadow: inset 0 0 0 1px @pearl;
    outline: none;
}

/* Buttons: Steam's 2 px keys. Play is green, install/add is blue,
   everything else is a quiet graphite key. */
button {
    border-radius: 6px;
    transition: background 160ms ease-out, color 120ms ease-out, box-shadow 160ms ease-out;
}
button.circular { border-radius: 6px; }
button:focus-visible { outline: 2px solid @pearl_light; outline-offset: 1px; }
/* Pearl: a dark violet with a sheen that slides across on hover. */
/* Primary: a solid pearl key. The light comes from above (a soft top
   highlight and a vertical gradient), never from a diagonal band: at button
   size a diagonal stop reads as a seam, not a sheen. */
button.suggested-action {
    background-image: linear-gradient(to bottom, shade(@pearl, 1.12), @pearl);
    color: #ffffff;
    font-weight: 700;
    padding: 7px 22px;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.16), 0 1px 2px rgba(0, 0, 0, 0.45);
    transition: box-shadow 150ms ease-out;
}
button.suggested-action:hover {
    background-image: linear-gradient(to bottom, shade(@pearl, 1.22), shade(@pearl, 1.08));
}
button.suggested-action:active {
    background-image: linear-gradient(to bottom, shade(@pearl, 0.92), shade(@pearl, 0.92));
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.45);
}
button.suggested-action.play { font-size: 15px; padding: 8px 30px; }
button.suggested-action:disabled {
    background-image: none;
    background-color: @surface_hover;
    color: @muted_fg;
    box-shadow: none;
}
button.global-control { min-width: 32px; min-height: 32px; padding: 0; color: alpha(@window_fg_color, 0.78); }
button.global-control:hover { color: @window_fg_color; }

/* Filters: Steam's library tabs. */
.segmented { background: none; padding: 0; }
.segmented > button {
    border-radius: 0;
    background: none;
    box-shadow: none;
    padding: 6px 0 7px;
    margin-right: 22px;
    font-weight: 700;
    color: @muted_fg;
}
.segmented > button:hover { color: @window_fg_color; background: none; }
.segmented > button:checked {
    color: @window_fg_color;
    background: none;
    box-shadow: inset 0 -2px 0 @pearl_light;
}

/* Game backdrop: a veil keeps text readable; panels above turn to glass. */
.backdrop-scrim {
    background-image: linear-gradient(to bottom, @veil_top 0%, @veil_mid 42%, @veil_bottom 100%);
}
.has-backdrop .nav-rail { background-color: alpha(@sidebar_bg_color, 0.62); }
.has-backdrop .shelf { background-color: alpha(@window_bg_color, 0.36); }
.has-backdrop headerbar.global-header { background-color: alpha(@headerbar_bg_color, 0.55); }

/* Hero: Steam's library header, art full bleed with the play bar below. */
.hero-wrap, .hero, .hero-scrim { border-radius: 8px; }
.hero-wrap { box-shadow: 0 10px 30px -12px rgba(0, 0, 0, 0.8); }
.hero-scrim {
    background-image: linear-gradient(to top,
        rgba(0, 0, 0, 0.96) 0%,
        rgba(0, 0, 0, 0.55) 45%,
        rgba(0, 0, 0, 0.00) 100%),
        linear-gradient(to right, rgba(0, 0, 0, 0.55) 0%, rgba(0, 0, 0, 0.0) 60%);
}
.hero-panel {
    background-color: transparent;
    border: 0;
    padding: 0;
    margin: 24px;
    box-shadow: none;
}
.hero-panel label { color: #ffffff; }
.hero-meta, .hero-panel .dim-label { color: rgba(236, 235, 242, 0.72); }
.hero-panel button.flat,
.hero-panel menubutton > button {
    background-color: rgba(0, 0, 0, 0.55);
    color: #ecebf2;
    font-weight: 600;
    padding: 7px 16px;
    box-shadow: inset 0 0 0 1px rgba(183, 164, 255, 0.28);
}
.hero-panel button.flat:hover,
.hero-panel menubutton > button:hover {
    background-color: rgba(123, 92, 240, 0.30);
    box-shadow: inset 0 0 0 1px rgba(183, 164, 255, 0.6);
    color: #ffffff;
}
.hero-panel menubutton > button { padding: 7px 10px; }

.status-pill {
    border-radius: 2px;
    padding: 2px 7px;
    font-weight: 700;
    font-size: 11px;
}
.pill-ready { background-color: alpha(@success_color, 0.18); color: @success_color; }
.pill-planned { background-color: alpha(@warning_color, 0.18); color: @warning_color; }
.pill-warning { background-color: alpha(@error_color, 0.18); color: @error_color; }
.pill-disabled { background-color: alpha(@insensitive_fg_color, 0.18); color: @insensitive_fg_color; }
.pill-neutral { background-color: @surface_raise; color: @window_fg_color; }
.hero-panel .status-pill { background-color: rgba(0, 0, 0, 0.6); color: #ecebf2; }
.hero-panel .pill-ready { color: #8ee6b0; }

.boxed-list, list.boxed-list, .card {
    border-radius: 3px;
    box-shadow: none;
}

.depot-table {
    background-color: @card_bg_color;
    border: 1px solid @borders;
    border-radius: 3px;
}
.depot-table separator { background-color: @borders; }
.mono { font-family: monospace; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pearl_black_redefines_the_palette_and_one_accent() {
        for theme in [Theme::Dark, Theme::Light] {
            let css = css(theme);
            assert!(css.contains("@define-color window_bg_color"));
            assert!(css.contains("--accent-bg-color: @pearl;"));
            assert!(css.contains("@pearl_light"), "pearl sheen");
            assert!(css.contains(".hero-panel"));
            assert!(css.contains(".segmented"));
        }
    }

    #[test]
    fn dark_and_light_differ_only_in_palette() {
        let dark = css(Theme::Dark);
        let light = css(Theme::Light);
        assert_ne!(dark, light);
        assert!(dark.ends_with(THEME) && light.ends_with(THEME));
    }

    #[test]
    fn a_stored_name_round_trips() {
        assert_eq!(Theme::from_setting("light"), Theme::Light);
        assert_eq!(Theme::from_setting("dark"), Theme::Dark);
        assert_eq!(Theme::from_setting("nonsense"), Theme::Dark);
        assert_eq!(Theme::Dark.next().as_str(), "light");
        assert_eq!(Theme::Light.next().as_str(), "dark");
    }
}
