//! Obsidian Forge: native GTK surfaces, exact brand tokens, and restrained
//! motion. Layout stays in the shell and views; this provider owns color,
//! typography, component state, and theme switching.

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

pub fn css(theme: Theme) -> String {
    match theme {
        Theme::Dark => THEME.to_string(),
        Theme::Light => format!("{THEME}\n{LIGHT}"),
    }
}

pub const THEME: &str = r#"
@define-color window_bg_color #09090D;
@define-color window_fg_color #F4F2FA;
@define-color view_bg_color #09090D;
@define-color view_fg_color #F4F2FA;
@define-color sidebar_bg_color #0E0E14;
@define-color sidebar_fg_color #F4F2FA;
@define-color headerbar_bg_color #0E0E14;
@define-color headerbar_fg_color #F4F2FA;
@define-color card_bg_color #14141D;
@define-color card_fg_color #F4F2FA;
@define-color popover_bg_color #14141D;
@define-color dialog_bg_color #14141D;
@define-color theme_bg_color #09090D;
@define-color theme_fg_color #F4F2FA;
@define-color theme_text_color #F4F2FA;
@define-color accent_bg_color #8B5CF6;
@define-color accent_color #C4B5FD;
@define-color accent_fg_color #09090D;
@define-color destructive_bg_color #EF4444;
@define-color destructive_fg_color #09090D;
@define-color success_color #10B981;
@define-color warning_color #F59E0B;
@define-color error_color #EF4444;
@define-color borders #2A2935;
@define-color insensitive_fg_color #858092;

window, window * {
    font-family: "Inter", "Geist", "Noto Sans", "Adwaita Sans", sans-serif;
}
window {
    background-color: #09090D;
    background-image: none;
    color: #F4F2FA;
}
headerbar {
    background-color: #0E0E14;
    background-image: none;
    border-bottom: 1px solid #2A2935;
    box-shadow: none;
    color: #F4F2FA;
}
headerbar button, headerbar image { color: #D8D3E2; }
headerbar button {
    min-width: 32px;
    min-height: 32px;
}
.nav-rail, .shelf {
    background-color: #0E0E14;
    background-image: none;
    border-right: 1px solid #2A2935;
}

.brand-name {
    font-size: 20px;
    font-weight: 750;
    letter-spacing: -0.02em;
    color: #F4F2FA;
}
.page-title, .inspector-title {
    font-size: 26px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: #F4F2FA;
}
.inspector-title, .hero-title { font-size: 28px; }
.shelf-title { font-size: 16px; font-weight: 650; color: #F4F2FA; }
.section-title, .summary-title {
    font-size: 15px;
    font-weight: 650;
    color: #F4F2FA;
}
.hero-title {
    font-weight: 750;
    letter-spacing: -0.025em;
    color: #FFFFFF;
}
.game-name, .discover-title, .nav-label { font-weight: 600; }
.brand-sub, .footer-label, .section-note, .game-meta, .dim-label, .caption {
    color: #AAA5B8;
}
.mono {
    font-family: "JetBrains Mono", "Geist Mono", "Noto Sans Mono", "Adwaita Mono", monospace;
    font-size: 11.5px;
    color: #AAA5B8;
}
.engine-footer {
    border-top: 1px solid #2A2935;
    padding-top: 12px;
}
.footer-engine { font-size: 11px; font-weight: 600; }
.footer-version { font-size: 11px; font-weight: 400; }

.nav-row, .game-row, .discover-card, button {
    transition: background-color 160ms ease-out, color 160ms ease-out,
        border-color 160ms ease-out;
}
.nav-row { border-radius: 6px; }
.nav-row:hover { background-color: alpha(#8B5CF6, 0.06); }
.nav-row:selected {
    background-color: alpha(#8B5CF6, 0.07);
    background-image: none;
    box-shadow: inset 3px 0 0 #8B5CF6;
}
.nav-row:focus-visible {
    outline: 1px solid alpha(#C4B5FD, 0.80);
    outline-offset: -3px;
}
.nav-row:selected .nav-icon, .nav-row:selected .nav-label { color: #C4B5FD; }
.game-row { border-radius: 8px; }
.game-row:hover { background-color: alpha(#8B5CF6, 0.06); }
.game-row:selected {
    background-color: alpha(#8B5CF6, 0.14);
    background-image: none;
    box-shadow: inset 3px 0 0 #8B5CF6;
}

button.suggested-action, button.suggested {
    background-color: #8B5CF6;
    background-image: none;
    color: #09090D;
    border: 1px solid transparent;
    border-radius: 8px;
    padding: 8px 16px;
    min-height: 20px;
    font-weight: 700;
    box-shadow: none;
}
button.suggested-action:hover, button.suggested:hover {
    background-color: #A78BFA;
    color: #09090D;
    box-shadow: none;
}
button.suggested-action:active, button.suggested:active {
    background-color: #7C3AED;
    color: #FFFFFF;
    transition-duration: 120ms;
}
button.flat { border-radius: 8px; }
button.flat:hover { background-color: alpha(#8B5CF6, 0.10); }
button:focus-visible, entry:focus-within, searchentry:focus-within,
listbox row:focus-visible {
    outline: 2px solid #C4B5FD;
    outline-offset: 2px;
}

headerbar entry, searchentry {
    background-color: #14141D;
    color: #F4F2FA;
    border: 1px solid #3A3748;
    border-radius: 8px;
    box-shadow: none;
}
headerbar entry placeholder, searchentry placeholder { color: #858092; }
headerbar entry:focus-within, searchentry:focus-within {
    border-color: #C4B5FD;
    box-shadow: none;
}

.hero-wrap {
    border-radius: 16px;
    border: 1px solid #3A3748;
    box-shadow: none;
}
.hero { border-radius: 16px; }
.hero-scrim {
    background-image: linear-gradient(to top,
        rgba(9, 9, 13, 0.96) 0%,
        rgba(9, 9, 13, 0.64) 46%,
        rgba(9, 9, 13, 0.08) 100%);
    border-radius: 16px;
}
.hero-panel {
    background-color: transparent;
    border: 0;
    border-radius: 0;
    padding: 0;
    margin: 24px;
    box-shadow: none;
}
.hero-panel label { color: #F4F2FA; }
.hero-meta { font-size: 12px; color: #D8D3E2; }
.hero-panel button.flat {
    background-color: rgba(255, 255, 255, 0.10);
    border: 1px solid rgba(255, 255, 255, 0.16);
    color: #F4F2FA;
}
.hero-panel button.flat:hover { background-color: rgba(255, 255, 255, 0.16); }

.depot-table, .summary-card {
    background-color: #14141D;
    background-image: none;
    border: 1px solid #2A2935;
    border-radius: 12px;
    box-shadow: none;
}
.depot-table separator { background-color: #2A2935; }
.depot-row:hover { background-color: alpha(#8B5CF6, 0.05); }

.status-pill {
    border-radius: 999px;
    padding: 3px 9px;
    font-size: 11px;
    font-weight: 700;
}
.count-pill, .pill-neutral { background-color: alpha(#8B5CF6, 0.15); color: #C4B5FD; }
.pill-ready { background-color: alpha(#10B981, 0.13); color: #10B981; }
.pill-planned { background-color: alpha(#F59E0B, 0.13); color: #F59E0B; }
.pill-warning { background-color: alpha(#EF4444, 0.13); color: #EF4444; }
.hero-scrim .status-pill { background-color: rgba(9, 9, 13, 0.78); }

.discover-card { border-radius: 12px; }
.discover-card:hover {
    background-color: alpha(#8B5CF6, 0.06);
    box-shadow: none;
}
.discover-cover { border-radius: 8px; }
.discover-action { color: #C4B5FD; font-size: 12px; font-weight: 600; }
.dot-ready { background-color: #10B981; animation: none; }
.dot-planned { background-color: #F59E0B; }
.dot-warning { background-color: #EF4444; }
scrollbar slider {
    background-color: alpha(#AAA5B8, 0.28);
    border-radius: 99px;
    min-width: 6px;
}
scrollbar slider:hover { background-color: alpha(#AAA5B8, 0.44); }
popover > contents { background-color: #14141D; border-color: #3A3748; }
list.boxed-list { background-color: #14141D; border-color: #2A2935; }
"#;

pub const LIGHT: &str = r#"
@define-color window_bg_color #F6F5F8;
@define-color window_fg_color #211D29;
@define-color view_bg_color #F6F5F8;
@define-color view_fg_color #211D29;
@define-color sidebar_bg_color #F0EDF4;
@define-color sidebar_fg_color #211D29;
@define-color headerbar_bg_color #F0EDF4;
@define-color headerbar_fg_color #211D29;
@define-color card_bg_color #FFFFFF;
@define-color card_fg_color #211D29;
@define-color popover_bg_color #FFFFFF;
@define-color dialog_bg_color #FFFFFF;
@define-color theme_bg_color #F6F5F8;
@define-color theme_fg_color #211D29;
@define-color theme_text_color #211D29;
@define-color accent_bg_color #7C3AED;
@define-color accent_color #6D28D9;
@define-color accent_fg_color #FFFFFF;
@define-color destructive_bg_color #C81E3A;
@define-color destructive_fg_color #FFFFFF;
@define-color borders #DCD7E3;
@define-color insensitive_fg_color #71697E;
@define-color success_color #087A55;
@define-color warning_color #8A5300;
@define-color error_color #C81E3A;

window { background-color: #F6F5F8; background-image: none; color: #211D29; }
headerbar {
    background-color: #F0EDF4;
    border-bottom: 1px solid #DCD7E3;
    color: #211D29;
}
headerbar button, headerbar image { color: #625B6D; }
.nav-rail, .shelf {
    background-color: #F0EDF4;
    border-right: 1px solid #DCD7E3;
}
.brand-name, .shelf-title, .section-title, .page-title, .game-name,
.discover-title, .nav-label, .inspector-title, .summary-title { color: #211D29; }
.brand-sub, .footer-label, .section-note, .game-meta, .mono, .dim-label, .caption {
    color: #625B6D;
}
.mono { color: #625B6D; }
.engine-footer { border-top-color: #DCD7E3; }
.nav-row:hover { background-color: alpha(#7C3AED, 0.05); }
.nav-row:hover .nav-label, .nav-row:hover .nav-icon { color: #6D28D9; }
.nav-row:selected {
    background-color: alpha(#7C3AED, 0.08);
    box-shadow: inset 3px 0 0 #7C3AED;
}
.nav-row:focus-visible {
    outline: 1px solid alpha(#6D28D9, 0.80);
    outline-offset: -3px;
}
.nav-row:selected .nav-icon, .nav-row:selected .nav-label { color: #6D28D9; }
.game-row:hover, .depot-row:hover, button.flat:hover,
.discover-card:hover { background-color: alpha(#7C3AED, 0.06); }
.game-row:selected {
    background-color: alpha(#7C3AED, 0.11);
    box-shadow: inset 3px 0 0 #7C3AED;
}
.count-pill, .pill-neutral { background-color: alpha(#7C3AED, 0.11); color: #6D28D9; }

button.suggested-action, button.suggested {
    background-color: #7C3AED;
    color: #FFFFFF;
    border-color: transparent;
    box-shadow: none;
}
button.suggested-action:hover, button.suggested:hover {
    background-color: #8B5CF6;
    color: #09090D;
}
button.suggested-action:active, button.suggested:active {
    background-color: #6D28D9;
    color: #FFFFFF;
}
button:focus-visible, entry:focus-within, searchentry:focus-within,
listbox row:focus-visible { outline-color: #6D28D9; }
headerbar entry, searchentry {
    background-color: #FFFFFF;
    border: 1px solid #C9C2D2;
    color: #211D29;
}
headerbar entry placeholder, searchentry placeholder { color: #71697E; }
headerbar entry:focus-within, searchentry:focus-within {
    border-color: #6D28D9;
    box-shadow: none;
}

.hero-wrap { border-color: #3A3748; }
.hero-panel label { color: #F4F2FA; }
.hero-panel .dim-label, .hero-panel .mono, .hero-meta { color: #DDD6E7; }
.hero-panel button.flat {
    color: #F4F2FA;
    background-color: rgba(255, 255, 255, 0.10);
    border-color: rgba(255, 255, 255, 0.16);
}
.hero-scrim .status-pill { background-color: rgba(9, 9, 13, 0.78); color: #F4F2FA; }
.depot-table, .summary-card {
    background-color: #FFFFFF;
    border: 1px solid #DCD7E3;
    box-shadow: none;
}
.depot-table separator { background-color: #DCD7E3; }
.pill-ready { background-color: alpha(#087A55, 0.11); color: #087A55; }
.pill-planned { background-color: alpha(#8A5300, 0.11); color: #8A5300; }
.pill-warning { background-color: alpha(#C81E3A, 0.11); color: #C81E3A; }
.status-dot.dot-ready { background-color: #087A55; }
.status-dot.dot-planned { background-color: #8A5300; }
.status-dot.dot-warning { background-color: #C81E3A; }
.discover-action { color: #6D28D9; }
scrollbar slider { background-color: alpha(#625B6D, 0.28); }
scrollbar slider:hover { background-color: alpha(#625B6D, 0.44); }
popover > contents { background-color: #FFFFFF; border-color: #C9C2D2; }
list.boxed-list { background-color: #FFFFFF; border-color: #DCD7E3; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obsidian_forge_tokens_and_stacks_are_present() {
        let dark = css(Theme::Dark);
        for token in [
            "#09090D", "#0E0E14", "#14141D", "#8B5CF6", "#7C3AED", "#10B981", "#F59E0B", "#EF4444",
        ] {
            assert!(dark.contains(token), "missing Obsidian Forge token {token}");
        }
        assert!(dark.contains("\"Inter\", \"Geist\", \"Noto Sans\", \"Adwaita Sans\""));
        assert!(dark
            .contains("\"JetBrains Mono\", \"Geist Mono\", \"Noto Sans Mono\", \"Adwaita Mono\""));
        assert!(dark.contains(".nav-row:selected"));
        assert!(dark.contains("box-shadow: inset 3px 0 0 #8B5CF6"));
        assert!(dark.contains(".hero-panel"));
    }

    #[test]
    fn both_themes_are_complete_and_different() {
        let dark = css(Theme::Dark);
        let light = css(Theme::Light);
        assert!(light.contains("#F6F5F8"));
        assert!(light.contains(".hero-panel"));
        assert!(light.contains("box-shadow: inset 3px 0 0 #7C3AED"));
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
