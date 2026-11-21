//! Midnight styling: a deep neutral palette expressed through libadwaita's
//! named colors, plus the component treatments (pill primary, glass hero
//! actions, segmented filters) that give the dashboard its own character.

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

/// Midnight: deep neutral layers in dark, soft paper in light. Only the
/// libadwaita named colors are redefined, so every native widget follows; the
/// accent stays the user's system accent.
pub fn css(theme: Theme) -> String {
    let palette = match theme {
        Theme::Dark => DARK,
        Theme::Light => LIGHT,
    };
    format!("{palette}{THEME}")
}

const DARK: &str = r#"
@define-color window_bg_color #0d0e12;
@define-color view_bg_color #0d0e12;
@define-color headerbar_bg_color #0d0e12;
@define-color headerbar_backdrop_color #0d0e12;
@define-color sidebar_bg_color #111217;
@define-color sidebar_backdrop_color #111217;
@define-color card_bg_color rgba(255, 255, 255, 0.045);
@define-color popover_bg_color #1a1b21;
@define-color dialog_bg_color #17181d;
@define-color borders rgba(255, 255, 255, 0.07);
@define-color surface_raise rgba(255, 255, 255, 0.06);
@define-color surface_hover rgba(255, 255, 255, 0.09);
"#;

const LIGHT: &str = r#"
@define-color window_bg_color #f5f6f8;
@define-color view_bg_color #f5f6f8;
@define-color headerbar_bg_color #f5f6f8;
@define-color headerbar_backdrop_color #f5f6f8;
@define-color sidebar_bg_color #eceef2;
@define-color sidebar_backdrop_color #eceef2;
@define-color card_bg_color #ffffff;
@define-color popover_bg_color #ffffff;
@define-color dialog_bg_color #ffffff;
@define-color borders rgba(0, 0, 0, 0.08);
@define-color surface_raise rgba(0, 0, 0, 0.05);
@define-color surface_hover rgba(0, 0, 0, 0.08);
"#;

pub const THEME: &str = r#"
/* Typography: tighter, heavier display titles. */
.title-1 {
    font-size: 28px;
    font-weight: 800;
    letter-spacing: -0.5px;
}
.title-3 { font-weight: 800; letter-spacing: -0.2px; }

/* Header: borderless, with a soft pill search. */
headerbar.global-header {
    box-shadow: none;
    border-bottom: 1px solid @borders;
    min-height: 52px;
}
entry.global-search, searchbar entry, .global-search {
    border-radius: 999px;
    background-color: @surface_raise;
    box-shadow: none;
    min-height: 34px;
    padding: 0 14px;
}
.global-search:focus-within {
    background-color: @surface_hover;
    outline: 2px solid alpha(@accent_bg_color, 0.55);
    outline-offset: -2px;
}

/* Buttons: rounded, quiet secondaries, a pill primary with a soft glow. */
button { border-radius: 10px; }
button.circular { border-radius: 999px; }
button.suggested-action {
    border-radius: 999px;
    padding: 8px 22px;
    font-weight: 700;
    box-shadow: 0 6px 18px -6px alpha(@accent_bg_color, 0.75);
    transition: box-shadow 160ms ease-out, background-color 160ms ease-out;
}
button.suggested-action:hover {
    box-shadow: 0 8px 24px -6px alpha(@accent_bg_color, 0.95);
}
button.global-control { border-radius: 999px; min-width: 34px; min-height: 34px; }

/* Segmented filters. */
.segmented {
    background-color: @surface_raise;
    border-radius: 999px;
    padding: 3px;
}
.segmented > button {
    border-radius: 999px;
    background: none;
    box-shadow: none;
    padding: 5px 16px;
    font-weight: 600;
    color: alpha(@window_fg_color, 0.65);
    transition: background-color 160ms ease-out, color 160ms ease-out;
}
.segmented > button:hover { color: @window_fg_color; }
.segmented > button:checked {
    background-color: @card_bg_color;
    color: @window_fg_color;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25), inset 0 0 0 1px @borders;
}

/* Hero: cinematic band, glass secondaries. */
.hero-wrap, .hero, .hero-scrim { border-radius: 16px; }
.hero-wrap { box-shadow: 0 18px 40px -18px rgba(0, 0, 0, 0.65); }
.hero-scrim {
    background-image: linear-gradient(to top,
        rgba(0, 0, 0, 0.92) 0%,
        rgba(0, 0, 0, 0.45) 48%,
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
.hero-panel label { color: #FFFFFF; }
.hero-title { font-size: 32px; font-weight: 800; letter-spacing: -0.6px; }
.hero-meta, .hero-panel .dim-label { color: rgba(255, 255, 255, 0.72); }
.hero-panel button.flat {
    background-color: rgba(255, 255, 255, 0.13);
    border: none;
    border-radius: 999px;
    padding: 8px 18px;
    font-weight: 600;
    color: #FFFFFF;
    transition: background-color 160ms ease-out;
}
.hero-panel button.flat:hover { background-color: rgba(255, 255, 255, 0.22); }
.hero-panel menubutton > button.flat,
.hero-panel menubutton > button { padding: 8px; min-width: 20px; border-radius: 999px; }

.status-pill {
    border-radius: 999px;
    padding: 3px 10px;
    font-weight: 700;
    font-size: 12px;
}
.pill-ready { background-color: alpha(@success_color, 0.14); color: @success_color; }
.pill-planned { background-color: alpha(@warning_color, 0.14); color: @warning_color; }
.pill-warning { background-color: alpha(@error_color, 0.14); color: @error_color; }
.pill-disabled { background-color: alpha(@insensitive_fg_color, 0.14); color: @insensitive_fg_color; }
.pill-neutral { background-color: alpha(@accent_bg_color, 0.14); color: @accent_color; }
.hero-scrim .status-pill, .hero-panel .status-pill {
    background-color: rgba(255, 255, 255, 0.14);
    color: #FFFFFF;
}
.hero-panel .pill-ready { background-color: alpha(@success_color, 0.28); }

/* Raised groups (expanders, preferences) lose the hard border. */
.boxed-list, list.boxed-list, .card {
    border-radius: 14px;
    box-shadow: inset 0 0 0 1px @borders;
}

.depot-table {
    background-color: @card_bg_color;
    border: 1px solid @borders;
    border-radius: 12px;
}
.depot-table separator { background-color: @borders; }
.mono { font-family: monospace; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midnight_redefines_adwaita_named_colors_and_keeps_the_system_accent() {
        for theme in [Theme::Dark, Theme::Light] {
            let css = css(theme);
            assert!(css.contains("@define-color window_bg_color"));
            assert!(css.contains("@define-color card_bg_color"));
            assert!(!css.contains("@define-color accent"));
            assert!(!css.contains("Inter"));
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
