//! "Nacre" theme: pure black base, deep violet pearlescent accents.
//! Loaded after the base STYLE so it overrides colours, type and motion.

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
/* ---------- palette ---------- */
@define-color window_bg_color #000000;
@define-color window_fg_color #ece8f6;
@define-color view_bg_color #000000;
@define-color view_fg_color #ece8f6;
@define-color sidebar_bg_color #050408;
@define-color headerbar_bg_color #000000;
@define-color headerbar_fg_color #ece8f6;
@define-color card_bg_color #0b0912;
@define-color card_fg_color #ece8f6;
@define-color popover_bg_color #0e0b17;
@define-color dialog_bg_color #0b0912;
@define-color accent_bg_color #7c5cff;
@define-color accent_color #b9a4ff;
@define-color accent_fg_color #ffffff;
@define-color success_color #7ee0c3;
@define-color warning_color #f2c78b;
@define-color error_color #ff8aa8;
@define-color borders alpha(#c9b8ff, 0.14);

/* ---------- type ---------- */
window, window * {
    font-family: "Noto Sans";
}
.mono {
    font-family: "Noto Sans";
    font-size: 12px;
    font-weight: 500;
    color: #d5c8e8;
}
.inspector-title, .page-title { font-size: 30px; font-weight: 800; }
.brand-name { font-size: 18px; font-weight: 800; }
.shelf-title { font-size: 17px; font-weight: 700; }
.section-title { font-size: 14px; font-weight: 600; color: #d9ccff; }
.game-name, .discover-title, .nav-label { font-weight: 600; }
.brand-sub, .footer-label, .section-note, .game-meta { color: #cfc2e4; }
.hero-scrim {
    background-image: linear-gradient(to top,
        rgba(0, 0, 0, 0.92) 0%,
        rgba(8, 3, 16, 0.58) 44%,
        rgba(10, 4, 20, 0.10) 72%,
        rgba(0, 0, 0, 0.01) 100%);
    border-radius: 18px;
}
.hero-panel button.flat {
    background-color: rgba(255, 255, 255, 0.10);
    border: 1px solid rgba(255, 255, 255, 0.18);
    color: #f6f1ff;
}
.hero-panel .status-pill { border: 1px solid rgba(255, 255, 255, 0.14); }
.hero-panel {
    background-color: rgba(4, 1, 9, 0.82);
    border: 1px solid alpha(#ffffff, 0.12);
    border-radius: 16px;
    padding: 12px 16px;
    margin: 14px;
    box-shadow: 0 10px 30px alpha(#000000, 0.45), inset 0 1px 0 alpha(#ffffff, 0.08);
}
.hero-title { font-size: 30px; font-weight: 800; color: #ffffff; margin-top: 4px; }
.hero-meta { font-size: 12.5px; color: #f3ecff; }
.hero-wrap { border-radius: 18px; box-shadow: 0 18px 52px alpha(#2b1350, 0.32), inset 0 1px 0 alpha(#ffffff, 0.10); }

/* ---------- surfaces ---------- */
window {
    background-color: #000000;
    background-image: linear-gradient(145deg, #040107 0%, #000000 42%, #05020a 100%);
}
.nav-rail {
    background-image: linear-gradient(180deg, #0a0612 0%, #030106 72%, #07030d 100%);
    border-right: 1px solid alpha(#c9b8ff, 0.12);
}
.shelf {
    background-color: #010002;
    background-image: linear-gradient(180deg, alpha(#1a0d2e, 0.42) 0%, alpha(#000000, 0.0) 42%);
    border-right: 1px solid alpha(#c9b8ff, 0.12);
}
headerbar { background: #000000; box-shadow: none; border-bottom: 1px solid alpha(#c9b8ff, 0.10); }

.depot-table, .summary-card {
    background-image: linear-gradient(145deg,
        alpha(#8e6bff, 0.16) 0%,
        alpha(#1a0e2b, 0.82) 34%,
        alpha(#050208, 0.96) 68%,
        alpha(#3a1d5e, 0.28) 100%);
    border: 1px solid alpha(#d3c6ff, 0.18);
    border-radius: 16px;
    box-shadow: 0 16px 44px alpha(#000000, 0.62), inset 0 1px 0 alpha(#ffffff, 0.07);
}

/* ---------- pearl: selection, pills, nav ---------- */
.nav-row, .game-row, .discover-card, button, .status-pill, .count-pill {
    transition: background-color 220ms ease, background-image 220ms ease,
                box-shadow 260ms ease, color 200ms ease, opacity 200ms ease;
}
.nav-row { border-radius: 10px; }
.nav-row:hover { background-color: alpha(#b9a4ff, 0.07); }
.nav-row:selected {
    background-image: linear-gradient(120deg, alpha(#8f6bff, 0.30), alpha(#7fd9ff, 0.10));
    box-shadow: inset 0 0 0 1px alpha(#d3c6ff, 0.22);
}
.game-row { border-radius: 12px; }
.game-row:hover { background-color: alpha(#b9a4ff, 0.06); }
.game-row:selected {
    background-image: linear-gradient(120deg, alpha(#8f6bff, 0.26), alpha(#ff9ee6, 0.08), alpha(#7fd9ff, 0.10));
    box-shadow: inset 0 0 0 1px alpha(#d3c6ff, 0.20), 0 6px 20px alpha(#7c5cff, 0.18);
}
.count-pill, .pill-neutral { background-color: alpha(#9b7bff, 0.18); color: #cdbfff; }
.pill-ready { background-color: alpha(#7ee0c3, 0.16); color: #a8f0d4; border: 1px solid alpha(#7ee0c3, 0.20); }
.status-pill { border-radius: 999px; padding: 2px 10px; font-weight: 700; }
.hero-scrim .status-pill { background-color: rgba(5, 2, 10, 0.72); border-color: rgba(255, 255, 255, 0.16); }

/* ---------- buttons ---------- */
button.suggested-action, button.suggested {
    background-image: linear-gradient(115deg,
        #1d0d2f 0%,
        #43256f 34%,
        #8f6be0 49%,
        #d9c9ff 55%,
        #6b47c4 68%,
        #1d0d2f 100%);
    color: #ffffff;
    font-weight: 700;
    border: 1px solid alpha(#ffffff, 0.22);
    border-radius: 999px;
    padding: 7px 20px;
    box-shadow: 0 8px 24px alpha(#5a3aa0, 0.30), inset 0 1px 0 alpha(#ffffff, 0.32);
}
button.suggested-action:hover, button.suggested:hover {
    box-shadow: 0 12px 32px alpha(#7c5cff, 0.40), inset 0 1px 0 alpha(#ffffff, 0.44);
}
button.suggested-action:active, button.suggested:active { opacity: 0.88; }
button.flat:hover { background-color: alpha(#b9a4ff, 0.10); }

/* ---------- search ---------- */
headerbar entry placeholder, searchentry placeholder { color: #c2b4d8; }
headerbar entry, searchentry {
    background-color: #0b0912;
    border-radius: 14px;
    border: 1px solid alpha(#d3c6ff, 0.14);
    box-shadow: none;
    outline: none;
    transition: border-color 240ms ease, box-shadow 280ms ease;
}
headerbar entry:focus-within, searchentry:focus-within {
    border-color: alpha(#b9a4ff, 0.55);
    box-shadow: 0 0 0 4px alpha(#7c5cff, 0.16), 0 0 28px alpha(#9b7bff, 0.25);
}

/* ---------- covers & motion ---------- */
.hero { border-radius: 18px; }
.discover-card:hover { background-color: transparent; box-shadow: 0 14px 34px alpha(#7c5cff, 0.30); }
.discover-cover, .shelf-cover { border-radius: 10px; }

@keyframes breathe {
    0%   { box-shadow: 0 0 0 0 alpha(#7ee0c3, 0.55); }
    70%  { box-shadow: 0 0 0 7px alpha(#7ee0c3, 0.0); }
    100% { box-shadow: 0 0 0 0 alpha(#7ee0c3, 0.0); }
}
.dot-ready { background-color: #7ee0c3; animation: breathe 2.4s ease-out infinite; }

scrollbar slider { background-color: alpha(#c9b8ff, 0.22); border-radius: 99px; min-width: 6px; }
scrollbar slider:hover { background-color: alpha(#c9b8ff, 0.40); }
"#;

pub const LIGHT: &str = r#"
@define-color window_bg_color #F0E5FF;
@define-color view_bg_color #FFFFFF;
@define-color headerbar_bg_color #FBF7FF;
@define-color sidebar_bg_color #FFFFFF;
@define-color card_bg_color #FFFFFF;
@define-color popover_bg_color #FFFFFF;
@define-color theme_bg_color #F0E5FF;
@define-color theme_fg_color #1A0B2E;
@define-color theme_text_color #1A0B2E;
@define-color accent_bg_color #6D3FD1;
@define-color accent_fg_color #FFFFFF;
@define-color accent_color #6D28D9;
@define-color borders alpha(#5A328C, 0.22);
@define-color insensitive_fg_color #3F3354;
@define-color success_color #2F8F6B;
@define-color warning_color #B7791F;
@define-color error_color #C2415A;

window {
    background-color: #F0E5FF;
    background-image: linear-gradient(145deg, #FCF7FF 0%, #F0E5FF 46%, #E2D0FF 100%);
    color: #1A0B2E;
}
headerbar { background: #FBF7FF; border-bottom: 1px solid alpha(#5A328C, 0.10); }
.nav-rail {
    background-image: linear-gradient(180deg, #FFFFFF 0%, #F5ECFF 72%, #EADDFF 100%);
    border-right: 1px solid alpha(#5A328C, 0.15);
}
.shelf {
    background-color: #F8F1FF;
    background-image: linear-gradient(180deg, alpha(#7C3AED, 0.08) 0%, alpha(#FFFFFF, 0.0) 44%);
    border-right: 1px solid alpha(#5A328C, 0.15);
}
window label { color: #1A0B2E; }
.brand-name, .shelf-title, .section-title, .page-title, .game-name, .discover-title, .nav-label, .inspector-title, .summary-title { color: #1A0B2E; }
.brand-sub, .footer-label, .section-note, .game-meta, .mono, .dim-label, .caption { color: #3F3354; }
.game-meta, .footer-label, .section-note, .mono, .caption { font-size: 12.5px; }
.dim-label { opacity: 1; }
headerbar button, headerbar image { color: #3F3354; }
.hero-panel label { color: #F6F1FF; }
.hero-panel .dim-label, .hero-panel .mono { color: #D8CDF0; }
.hero-panel button.flat {
    color: #F6F1FF;
    background-color: rgba(255, 255, 255, 0.10);
    border: 1px solid rgba(255, 255, 255, 0.16);
}
.hero-panel .pill-ready { background-color: rgba(126, 224, 195, 0.18); color: #B8F4DF; }
.hero-panel .pill-planned { background-color: rgba(242, 199, 139, 0.18); color: #FFDFB2; }
.hero-panel .pill-warning { background-color: rgba(255, 138, 168, 0.18); color: #FFC2D2; }
.hero-panel .pill-disabled { background-color: rgba(255, 255, 255, 0.10); color: #DDD2F0; }
.nav-row:hover, .game-row:hover, .depot-row:hover, button.flat:hover { background-color: alpha(#6D3FD1, 0.08); }
.nav-row:selected, .game-row:selected {
    background-image: linear-gradient(90deg, alpha(#7C3AED, 0.16), alpha(#7C3AED, 0.02));
    box-shadow: inset 0 0 0 1px alpha(#7C3AED, 0.14);
}
.nav-row:selected .nav-icon, .nav-row:selected .nav-label { color: #5B21B6; }
.count-pill { background-image: linear-gradient(135deg, #FFFFFF, #C9B6FF); color: #1A0B2E; border: 1px solid alpha(#7C3AED, 0.18); }
.depot-table, .summary-card {
    background-image: linear-gradient(145deg, #FFFFFF 0%, #F8F2FF 55%, #EEE4FF 100%);
    border: 1px solid alpha(#5A328C, 0.22);
    box-shadow: 0 14px 36px alpha(#5A328C, 0.12), inset 0 1px 0 #FFFFFF;
}
.depot-table separator { background-color: alpha(#5A328C, 0.22); }
.depot-table .caption { color: #3F3354; }
preferencespage, .preferences-page { color: #1A0B2E; }
.dim-label { color: #5E5174; }
.status-dot.dot-ready { background-color: #2F8F6B; }
.status-dot.dot-planned { background-color: #B7791F; }
.status-dot.dot-warning { background-color: #C2415A; }
.pill-ready { background-color: alpha(#2F8F6B, 0.13); color: #237A59; border-color: alpha(#2F8F6B, 0.20); }
.pill-planned { background-color: alpha(#B7791F, 0.13); color: #8A5A13; }
.pill-warning { background-color: alpha(#C2415A, 0.13); color: #A32E45; }
.hero-scrim .status-pill { background-color: rgba(5, 2, 10, 0.72); color: #F4ECFF; }
button.suggested-action, button.suggested {
    background-image: linear-gradient(115deg, #2A1445 0%, #5B3BB5 38%, #D7C6FF 53%, #7C5CFF 68%, #2A1445 100%);
    color: #FFFFFF;
    border-color: alpha(#FFFFFF, 0.36);
    box-shadow: 0 10px 28px alpha(#6D3FD1, 0.28), inset 0 1px 0 alpha(#FFFFFF, 0.42);
}
headerbar entry, searchentry {
    background-color: #FFFFFF;
    border: 1px solid alpha(#5A328C, 0.28);
    color: #1A0B2E;
}
headerbar entry placeholder, searchentry placeholder { color: #564A6B; }
headerbar entry:focus-within, searchentry:focus-within {
    border-color: alpha(#6D3FD1, 0.60);
    box-shadow: 0 0 0 4px alpha(#6D3FD1, 0.10);
}
.discover-card:hover { background-color: alpha(#6D3FD1, 0.07); }
.scrollbar slider, scrollbar slider { background-color: alpha(#5A328C, 0.24); }
popover > contents { background-color: #FFFFFF; border-color: alpha(#5A328C, 0.16); }
list.boxed-list { background-color: #FFFFFF; border-color: alpha(#5A328C, 0.22); }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_themes_are_complete_and_different() {
        let dark = css(Theme::Dark);
        let light = css(Theme::Light);
        assert!(dark.contains("#000000"));
        assert!(dark.contains(".hero-panel"));
        assert!(light.contains("#F0E5FF"));
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
