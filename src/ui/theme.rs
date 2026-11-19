//! Native Adwaita styling. The platform owns the palette and typography; this
//! provider only supplies artwork treatments, semantic status and small layout
//! details that the toolkit does not provide itself.

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

pub fn css(_theme: Theme) -> String {
    THEME.to_string()
}

pub const THEME: &str = r#"
/* Adwaita supplies window_bg_color, accent_color, fonts and control states. */
.hero-wrap, .hero, .hero-scrim { border-radius: 12px; }
.hero-scrim {
    background-image: linear-gradient(to top,
        rgba(0, 0, 0, 0.90) 0%,
        rgba(0, 0, 0, 0.38) 52%,
        rgba(0, 0, 0, 0.02) 100%);
}
.hero-panel {
    background-color: transparent;
    border: 0;
    padding: 0;
    margin: 22px;
    box-shadow: none;
}
.hero-panel label { color: #FFFFFF; }
.hero-meta, .hero-panel .dim-label { color: rgba(255, 255, 255, 0.78); }
.hero-panel button.flat {
    background-color: rgba(255, 255, 255, 0.12);
    border: 1px solid rgba(255, 255, 255, 0.20);
    color: #FFFFFF;
}
.hero-panel button.flat:hover { background-color: rgba(255, 255, 255, 0.18); }

.status-pill {
    border-radius: 999px;
    padding: 3px 9px;
    font-weight: 600;
}
.pill-ready { background-color: alpha(@success_color, 0.14); color: @success_color; }
.pill-planned { background-color: alpha(@warning_color, 0.14); color: @warning_color; }
.pill-warning { background-color: alpha(@error_color, 0.14); color: @error_color; }
.pill-disabled { background-color: alpha(@insensitive_fg_color, 0.14); color: @insensitive_fg_color; }
.pill-neutral { background-color: alpha(@accent_bg_color, 0.14); color: @accent_color; }
.hero-scrim .status-pill { background-color: rgba(0, 0, 0, 0.52); color: #FFFFFF; }

.depot-table {
    background-color: @card_bg_color;
    border: 1px solid @borders;
    border-radius: 10px;
}
.depot-table separator { background-color: @borders; }
.mono { font-family: monospace; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_theme_uses_adwaita_instead_of_a_bespoke_palette() {
        let css = css(Theme::Dark);
        assert!(!css.contains("@define-color"));
        assert!(!css.contains("Inter"));
        assert!(!css.contains("Geist"));
        assert!(!css.contains("#8B5CF6"));
        assert!(css.contains("@success_color"));
        assert!(css.contains("@card_bg_color"));
        assert!(css.contains(".hero-panel"));
    }

    #[test]
    fn dark_and_light_share_the_same_native_component_rules() {
        assert_eq!(css(Theme::Dark), css(Theme::Light));
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
