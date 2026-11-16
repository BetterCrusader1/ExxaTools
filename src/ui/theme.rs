//! Nacre: quiet native surfaces with one violet accent and the game artwork
//! carrying the colour. Loaded after STYLE, so these rules own theme colours,
//! controls and motion without changing the layout widgets.

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
            Self::Dark => "Switch to the light pearl theme",
            Self::Light => "Switch to the dark pearl theme",
        }
    }

    pub fn color_scheme(self) -> adw::ColorScheme {
        match self {
            Self::Dark => adw::ColorScheme::ForceDark,
            Self::Light => adw::ColorScheme::ForceLight,
        }
    }
}

pub fn css(theme: Theme) -> String {
    match theme {
        Theme::Dark => THEME.to_string(),
        Theme::Light => format!("{THEME}\n{LIGHT}"),
    }
}

pub const THEME: &str = r#"
@define-color window_bg_color #0b0b10;
@define-color window_fg_color #ece9f1;
@define-color view_bg_color #0b0b10;
@define-color view_fg_color #ece9f1;
@define-color sidebar_bg_color #101017;
@define-color sidebar_fg_color #ece9f1;
@define-color headerbar_bg_color #0f0f15;
@define-color headerbar_fg_color #ece9f1;
@define-color card_bg_color #14141c;
@define-color card_fg_color #ece9f1;
@define-color popover_bg_color #191922;
@define-color dialog_bg_color #14141c;
@define-color accent_bg_color #8b7cf6;
@define-color accent_color #b7a8ff;
@define-color accent_fg_color #111018;
@define-color success_color #73caa8;
@define-color warning_color #d8b26e;
@define-color error_color #e68491;
@define-color borders alpha(#c9c2dd, 0.14);

window, window * { font-family: "Noto Sans"; }
window { background-color: #0b0b10; background-image: none; color: #ece9f1; }
headerbar {
    background: #0f0f15;
    border-bottom: 1px solid alpha(#c9c2dd, 0.10);
    box-shadow: none;
}
.nav-rail {
    background-color: #101017;
    background-image: none;
    border-right: 1px solid alpha(#c9c2dd, 0.11);
}
.shelf {
    background-color: #0e0e14;
    background-image: none;
    border-right: 1px solid alpha(#c9c2dd, 0.11);
}

.brand-name { font-size: 20px; font-weight: 750; letter-spacing: -0.02em; }
.page-title, .inspector-title { font-size: 26px; font-weight: 750; letter-spacing: -0.02em; }
.shelf-title { font-size: 16px; font-weight: 700; }
.section-title, .summary-title { font-size: 14px; font-weight: 700; color: #ece9f1; }
.hero-title { font-size: 28px; font-weight: 750; letter-spacing: -0.025em; color: #ffffff; }
.game-name, .discover-title, .nav-label { font-weight: 600; }
.brand-sub, .footer-label, .section-note, .game-meta, .dim-label, .caption { color: #aaa4b6; }
.mono { font-family: "Noto Sans Mono"; font-size: 11.5px; color: #c4bdd0; }

.nav-row, .game-row, .discover-card, button {
    transition: background-color 140ms ease, color 140ms ease, border-color 140ms ease;
}
.nav-row { border-radius: 8px; }
.nav-row:hover { background-color: alpha(#b7a8ff, 0.06); }
.nav-row:selected {
    background-color: alpha(#8b7cf6, 0.16);
    background-image: none;
    box-shadow: none;
}
.nav-row:selected .nav-icon, .nav-row:selected .nav-label { color: #d8ceff; }
.game-row { border-radius: 9px; }
.game-row:hover { background-color: alpha(#b7a8ff, 0.055); }
.game-row:selected {
    background-color: alpha(#8b7cf6, 0.15);
    background-image: none;
    box-shadow: inset 3px 0 0 #8b7cf6;
}

button.suggested-action, button.suggested {
    background-color: #8b7cf6;
    background-image: none;
    color: #111018;
    border: 1px solid transparent;
    border-radius: 10px;
    padding: 8px 16px;
    min-height: 20px;
    font-weight: 700;
    box-shadow: none;
}
button.suggested-action:hover, button.suggested:hover {
    background-color: #9d90f8;
    box-shadow: none;
}
button.suggested-action:active, button.suggested:active { background-color: #7b69e8; }
button.flat { border-radius: 8px; }
button.flat:hover { background-color: alpha(#b7a8ff, 0.09); }
button:focus-visible, entry:focus-within, searchentry:focus-within {
    outline: 2px solid alpha(#b7a8ff, 0.75);
    outline-offset: 2px;
}

headerbar entry, searchentry {
    background-color: #15151d;
    border: 1px solid alpha(#c9c2dd, 0.15);
    border-radius: 10px;
    box-shadow: none;
}
headerbar entry placeholder, searchentry placeholder { color: #8f899b; }
headerbar entry:focus-within, searchentry:focus-within {
    border-color: alpha(#b7a8ff, 0.60);
    box-shadow: none;
}

.hero-wrap {
    border-radius: 16px;
    border: 1px solid alpha(#ffffff, 0.10);
    box-shadow: none;
}
.hero { border-radius: 16px; }
.hero-scrim {
    background-image: linear-gradient(to top,
        rgba(5, 5, 9, 0.96) 0%,
        rgba(5, 5, 9, 0.64) 46%,
        rgba(5, 5, 9, 0.08) 100%);
    border-radius: 16px;
}
.hero-panel {
    background-color: transparent;
    border: 0;
    border-radius: 0;
    padding: 0;
    margin: 22px;
    box-shadow: none;
}
.hero-panel label { color: #f5f2fa; }
.hero-meta { font-size: 12px; color: #d8d2e2; }
.hero-panel button.flat {
    background-color: rgba(255, 255, 255, 0.10);
    border: 1px solid rgba(255, 255, 255, 0.16);
    color: #f5f2fa;
}
.hero-panel button.flat:hover { background-color: rgba(255, 255, 255, 0.16); }

.depot-table, .summary-card {
    background-color: #14141c;
    background-image: none;
    border: 1px solid alpha(#c9c2dd, 0.13);
    border-radius: 12px;
    box-shadow: none;
}
.depot-table separator { background-color: alpha(#c9c2dd, 0.10); }
.depot-row:hover { background-color: alpha(#b7a8ff, 0.045); }

.status-pill {
    border-radius: 999px;
    padding: 3px 9px;
    font-size: 11px;
    font-weight: 700;
}
.count-pill, .pill-neutral { background-color: alpha(#8b7cf6, 0.15); color: #c8bcff; }
.pill-ready { background-color: alpha(#73caa8, 0.13); color: #91ddbd; }
.pill-planned { background-color: alpha(#d8b26e, 0.13); color: #e4c58c; }
.pill-warning { background-color: alpha(#e68491, 0.13); color: #efa3ae; }
.hero-scrim .status-pill { background-color: rgba(10, 10, 15, 0.74); }

.discover-card { border-radius: 12px; }
.discover-card:hover {
    background-color: alpha(#b7a8ff, 0.055);
    box-shadow: none;
}
.discover-cover { border-radius: 10px; }
.discover-action { color: #b7a8ff; font-size: 11.5px; font-weight: 700; }
.dot-ready { background-color: #73caa8; animation: none; }
scrollbar slider { background-color: alpha(#c9c2dd, 0.22); border-radius: 99px; min-width: 6px; }
scrollbar slider:hover { background-color: alpha(#c9c2dd, 0.38); }
popover > contents { background-color: #191922; border-color: alpha(#c9c2dd, 0.15); }
list.boxed-list { background-color: #14141c; border-color: alpha(#c9c2dd, 0.13); }
"#;

pub const LIGHT: &str = r#"
@define-color window_bg_color #f6f4f8;
@define-color window_fg_color #24202c;
@define-color view_bg_color #fbfafc;
@define-color view_fg_color #24202c;
@define-color sidebar_bg_color #f0edf5;
@define-color sidebar_fg_color #24202c;
@define-color headerbar_bg_color #fbfafc;
@define-color headerbar_fg_color #24202c;
@define-color card_bg_color #ffffff;
@define-color card_fg_color #24202c;
@define-color popover_bg_color #ffffff;
@define-color dialog_bg_color #ffffff;
@define-color theme_bg_color #f6f4f8;
@define-color theme_fg_color #24202c;
@define-color theme_text_color #24202c;
@define-color accent_bg_color #6d55d9;
@define-color accent_color #6248c7;
@define-color accent_fg_color #ffffff;
@define-color borders alpha(#514461, 0.16);
@define-color insensitive_fg_color #71697e;
@define-color success_color #2f8967;
@define-color warning_color #9a681d;
@define-color error_color #b8445a;

window { background-color: #f6f4f8; background-image: none; color: #24202c; }
headerbar { background: #fbfafc; border-bottom: 1px solid alpha(#514461, 0.11); }
.nav-rail { background-color: #f0edf5; background-image: none; border-right: 1px solid alpha(#514461, 0.12); }
.shelf { background-color: #f8f6fa; background-image: none; border-right: 1px solid alpha(#514461, 0.12); }
window label { color: #24202c; }
.brand-name, .shelf-title, .section-title, .page-title, .game-name,
.discover-title, .nav-label, .inspector-title, .summary-title { color: #24202c; }
.brand-sub, .footer-label, .section-note, .game-meta, .mono, .dim-label, .caption { color: #71697e; }
.mono { color: #574e65; }
headerbar button, headerbar image { color: #514861; }
.nav-row:hover { background-color: alpha(#6d55d9, 0.055); }
.nav-row:hover .nav-label, .nav-row:hover .nav-icon { color: #563daf; }
.nav-row:selected { background-color: alpha(#6d55d9, 0.12); }
.nav-row:selected .nav-icon, .nav-row:selected .nav-label { color: #4e369f; }
.game-row:hover, .depot-row:hover, button.flat:hover { background-color: alpha(#6d55d9, 0.055); }
.game-row:selected { background-color: alpha(#6d55d9, 0.11); box-shadow: inset 3px 0 0 #6d55d9; }
.count-pill { background-color: alpha(#6d55d9, 0.11); color: #563daf; border: 0; }

button.suggested-action, button.suggested {
    background-color: #6d55d9;
    background-image: none;
    color: #ffffff;
    border-color: transparent;
    box-shadow: none;
}
button.suggested-action:hover, button.suggested:hover { background-color: #7a63e2; }
button.suggested-action:active, button.suggested:active { background-color: #5d46c4; }
headerbar entry, searchentry {
    background-color: #ffffff;
    border: 1px solid alpha(#514461, 0.20);
    color: #24202c;
}
headerbar entry placeholder, searchentry placeholder { color: #827a8f; }
headerbar entry:focus-within, searchentry:focus-within { border-color: alpha(#6d55d9, 0.60); box-shadow: none; }

.hero-panel label { color: #f8f6fb; }
.hero-panel .dim-label, .hero-panel .mono, .hero-meta { color: #ddd6e7; }
.hero-panel button.flat { color: #f8f6fb; background-color: rgba(255,255,255,0.10); border-color: rgba(255,255,255,0.16); }
.hero-scrim .status-pill { background-color: rgba(10, 10, 15, 0.74); color: #f5f2fa; }
.depot-table, .summary-card {
    background-color: #ffffff;
    background-image: none;
    border: 1px solid alpha(#514461, 0.14);
    box-shadow: none;
}
.depot-table separator { background-color: alpha(#514461, 0.12); }
.pill-ready { background-color: alpha(#2f8967, 0.11); color: #277355; }
.pill-planned { background-color: alpha(#9a681d, 0.11); color: #805415; }
.pill-warning { background-color: alpha(#b8445a, 0.11); color: #9e374b; }
.status-dot.dot-ready { background-color: #2f8967; }
.status-dot.dot-planned { background-color: #9a681d; }
.status-dot.dot-warning { background-color: #b8445a; }
.discover-card:hover { background-color: alpha(#6d55d9, 0.055); }
.discover-action { color: #6248c7; }
scrollbar slider { background-color: alpha(#514461, 0.22); }
popover > contents { background-color: #ffffff; border-color: alpha(#514461, 0.15); }
list.boxed-list { background-color: #ffffff; border-color: alpha(#514461, 0.14); }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_themes_are_complete_and_different() {
        let dark = css(Theme::Dark);
        let light = css(Theme::Light);
        assert!(dark.contains("#0b0b10"));
        assert!(dark.contains(".hero-panel"));
        assert!(light.contains("#f6f4f8"));
        assert!(light.contains(".hero-panel"));
        assert_ne!(dark, light);
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
