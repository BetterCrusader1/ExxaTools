//! The selected game's blurred art behind the whole window, plus the game's
//! accent for the inspector. Panels turn translucent over it, which reads as
//! frosted glass because the art itself is already blurred.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;

use crate::ui::art::{self, Rgb};

/// Matches the page crossfade feel but slower: the backdrop is ambience.
const FADE_MS: u32 = 450;

pub struct Backdrop {
    pub root: gtk::Overlay,
    stack: gtk::Stack,
    pictures: [gtk::Picture; 2],
    front: Cell<usize>,
    host: gtk::Widget,
    accent: gtk::CssProvider,
    path: RefCell<Option<PathBuf>>,
    shown: Cell<bool>,
    active: Cell<bool>,
    generation: Cell<u64>,
}

impl Backdrop {
    /// `host` is the content root; it gets `.has-backdrop` while art shows so
    /// the panels above can turn translucent.
    pub fn new(host: &gtk::Widget) -> Rc<Self> {
        let pictures = [picture(), picture()];
        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(FADE_MS);
        stack.set_hhomogeneous(false);
        stack.set_vhomogeneous(false);
        stack.add_named(&gtk::Box::new(gtk::Orientation::Vertical, 0), Some("none"));
        stack.add_named(&pictures[0], Some("a"));
        stack.add_named(&pictures[1], Some("b"));
        stack.set_visible_child_name("none");

        let scrim = gtk::Box::new(gtk::Orientation::Vertical, 0);
        scrim.add_css_class("backdrop-scrim");
        scrim.set_can_target(false);

        let root = gtk::Overlay::new();
        root.add_css_class("backdrop");
        root.set_can_target(false);
        root.set_child(Some(&stack));
        root.add_overlay(&scrim);

        let accent = gtk::CssProvider::new();
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &accent,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 3,
            );
        }

        Rc::new(Self {
            root,
            stack,
            pictures,
            front: Cell::new(0),
            host: host.clone(),
            accent,
            path: RefCell::new(None),
            shown: Cell::new(false),
            active: Cell::new(true),
            generation: Cell::new(0),
        })
    }

    /// Only the Library shows a game backdrop; other pages stay plain.
    pub fn set_active(&self, active: bool) {
        self.active.set(active);
        self.sync_visibility();
    }

    /// Shows `path` (a hero or cover) blurred, or clears the backdrop. The
    /// same path twice is a no-op so a re-render does not refade.
    pub fn set_art(self: &Rc<Self>, path: Option<&Path>) {
        if self.path.borrow().as_deref() == path {
            return;
        }
        self.path.replace(path.map(Path::to_path_buf));
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        let Some(path) = path else {
            self.shown.set(false);
            set_accent(&self.accent, None);
            self.sync_visibility();
            return;
        };
        let this = self.clone();
        art::backdrop(path, move |loaded| {
            if this.generation.get() != generation {
                return;
            }
            let Some(loaded) = loaded else {
                this.shown.set(false);
                set_accent(&this.accent, None);
                this.sync_visibility();
                return;
            };
            let back = 1 - this.front.get();
            this.pictures[back].set_paintable(Some(&loaded.texture));
            this.front.set(back);
            this.shown.set(true);
            set_accent(&this.accent, loaded.accent);
            this.sync_visibility();
        });
    }

    fn sync_visibility(&self) {
        let visible = self.active.get() && self.shown.get();
        let name = match (visible, self.front.get()) {
            (false, _) => "none",
            (true, 0) => "a",
            (true, _) => "b",
        };
        self.stack.set_visible_child_name(name);
        if visible {
            self.host.add_css_class("has-backdrop");
        } else {
            self.host.remove_css_class("has-backdrop");
        }
    }
}

fn picture() -> gtk::Picture {
    let picture = gtk::Picture::new();
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_can_shrink(true);
    picture.set_hexpand(true);
    picture.set_vexpand(true);
    picture
}

/// libadwaita 1.6+ derives `--accent-color` from `--accent-bg-color`, so one
/// scoped variable recolors suggested buttons and accent text in `.game-tint`.
fn accent_css(accent: Option<Rgb>) -> String {
    match accent {
        Some(color) => format!(
            ".game-tint {{ --accent-bg-color: {}; --accent-fg-color: #ffffff; }}",
            color.hex()
        ),
        None => String::new(),
    }
}

fn set_accent(provider: &gtk::CssProvider, accent: Option<Rgb>) {
    provider.load_from_data(&accent_css(accent));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_accent_is_scoped_to_the_tinted_area() {
        let css = accent_css(Some(Rgb(0xac, 0x56, 0x22)));
        assert_eq!(
            css,
            ".game-tint { --accent-bg-color: #ac5622; --accent-fg-color: #ffffff; }"
        );
    }

    #[test]
    fn no_accent_clears_the_override() {
        assert!(accent_css(None).is_empty());
    }
}
