//! The Library as a collection of portrait cards. The selected card is the
//! only foil on the screen; every other card is plain card stock.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib, Box as GtkBox, Orientation};

use crate::ui::art::Art;
use crate::ui::model::Game;

const CARD_WIDTH: i32 = 168;
const CARD_HEIGHT: i32 = 252;

type SelectHook = Box<dyn Fn(Option<u64>)>;

#[derive(Clone)]
pub struct Collection {
    pub root: gtk::Widget,
    title: gtk::Label,
    subtitle: gtk::Label,
    stack: gtk::Stack,
    store: gio::ListStore,
    selection: gtk::SingleSelection,
    pub appids: Rc<RefCell<Vec<u64>>>,
    hook: Rc<RefCell<Option<SelectHook>>>,
    /// Programmatic selection must not echo back into a new select call.
    syncing: Rc<Cell<bool>>,
}

pub fn build(on_discover: impl Fn() + 'static) -> Collection {
    let title = gtk::Label::new(Some("Your games"));
    title.add_css_class("title-3");
    title.set_halign(gtk::Align::Start);
    let subtitle = gtk::Label::new(None);
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);
    subtitle.set_xalign(0.0);
    subtitle.set_wrap(true);

    let heading = GtkBox::new(Orientation::Vertical, 2);
    heading.set_margin_start(24);
    heading.set_margin_end(24);
    heading.set_margin_top(28);
    heading.append(&title);
    heading.append(&subtitle);

    let store = gio::ListStore::new::<glib::BoxedAnyObject>();
    let selection = gtk::SingleSelection::new(Some(store.clone()));
    selection.set_autoselect(false);
    selection.set_can_unselect(false);

    let grid = gtk::GridView::new(Some(selection.clone()), Some(card_factory()));
    grid.add_css_class("collection-grid");
    grid.set_min_columns(2);
    grid.set_max_columns(12);
    grid.set_single_click_activate(false);
    // The page scrolls as one; the grid shows every card it holds.
    grid.set_vscroll_policy(gtk::ScrollablePolicy::Natural);

    let empty = empty_state(on_discover);

    let stack = gtk::Stack::new();
    stack.set_vhomogeneous(false);
    stack.add_named(&grid, Some("grid"));
    stack.add_named(&empty, Some("empty"));
    stack.set_visible_child_name("empty");

    let root = GtkBox::new(Orientation::Vertical, 8);
    root.add_css_class("collection");
    root.append(&heading);
    root.append(&stack);

    let collection = Collection {
        root: root.upcast(),
        title,
        subtitle,
        stack,
        store,
        selection: selection.clone(),
        appids: Rc::new(RefCell::new(Vec::new())),
        hook: Rc::new(RefCell::new(None)),
        syncing: Rc::new(Cell::new(false)),
    };
    {
        let appids = collection.appids.clone();
        let hook = collection.hook.clone();
        let syncing = collection.syncing.clone();
        selection.connect_selected_notify(move |selection| {
            if syncing.get() {
                return;
            }
            let index = selection.selected();
            let appid = appids.borrow().get(index as usize).copied();
            if let Some(hook) = hook.borrow().as_ref() {
                hook(appid);
            }
        });
    }
    collection
}

impl Collection {
    pub fn connect_select(&self, hook: impl Fn(Option<u64>) + 'static) {
        self.hook.replace(Some(Box::new(hook)));
    }

    pub fn set_games(&self, games: &[Game]) {
        self.syncing.set(true);
        self.appids
            .replace(games.iter().map(|game| game.appid).collect());
        let items = games
            .iter()
            .cloned()
            .map(glib::BoxedAnyObject::new)
            .collect::<Vec<_>>();
        self.store.splice(0, self.store.n_items(), &items);
        self.selection.set_selected(gtk::INVALID_LIST_POSITION);
        self.syncing.set(false);
        self.stack
            .set_visible_child_name(if games.is_empty() { "empty" } else { "grid" });
    }

    pub fn set_title(&self, title: &str, subtitle: &str) {
        self.title.set_text(title);
        self.subtitle.set_text(subtitle);
        self.subtitle.set_visible(!subtitle.is_empty());
    }

    /// Selects without re-entering the select hook; the caller already acts.
    pub fn select_appid(&self, appid: u64) -> bool {
        let index = self.appids.borrow().iter().position(|value| *value == appid);
        match index {
            Some(index) => {
                self.selection.set_selected(index as u32);
                true
            }
            None => false,
        }
    }

    pub fn select_first(&self) {
        if self.store.n_items() > 0 {
            self.selection.set_selected(0);
        } else if let Some(hook) = self.hook.borrow().as_ref() {
            hook(None);
        }
    }
}

fn empty_state(on_discover: impl Fn() + 'static) -> gtk::Widget {
    let page = adw::StatusPage::builder()
        .icon_name("applications-games-symbolic")
        .title("Add your first game")
        .description("Pick a game in Discover or search Steam above. Steam downloads it like any other game.")
        .build();
    let button = gtk::Button::with_label("Browse Discover");
    button.add_css_class("suggested-action");
    button.set_halign(gtk::Align::Center);
    button.connect_clicked(move |_| on_discover());
    page.set_child(Some(&button));
    page.upcast()
}

/// Cells are recycled; each one rewrites its art and text on rebind.
fn card_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let art = Art::new(CARD_WIDTH, CARD_HEIGHT, "card-art");
        art.root.set_hexpand(false);

        let foil = GtkBox::new(Orientation::Vertical, 0);
        foil.add_css_class("foil");
        foil.set_can_target(false);

        let frame = gtk::Overlay::new();
        frame.add_css_class("card-frame");
        frame.set_overflow(gtk::Overflow::Hidden);
        frame.set_child(Some(&art.root));
        frame.add_overlay(&foil);
        frame.set_halign(gtk::Align::Center);

        let name = gtk::Label::new(None);
        name.add_css_class("card-name");
        name.set_xalign(0.0);
        // Two lines: the name is how a player finds a game whose logo the
        // poster crops; the ellipsis only cuts the third line.
        name.set_wrap(true);
        name.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        name.set_lines(2);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_max_width_chars(16);
        name.set_width_request(CARD_WIDTH - 4);
        name.set_valign(gtk::Align::Start);

        let caption = gtk::Label::new(None);
        caption.add_css_class("card-caption");
        caption.set_xalign(0.0);
        caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
        caption.set_max_width_chars(18);

        let card = GtkBox::new(Orientation::Vertical, 0);
        card.add_css_class("game-card");
        card.set_halign(gtk::Align::Center);
        card.append(&frame);
        card.append(&name);
        card.append(&caption);
        item.set_child(Some(&card));

        item.connect_item_notify(move |item| {
            let Some(object) = item.item().and_downcast::<glib::BoxedAnyObject>() else {
                art.set(None, "", None);
                return;
            };
            let game = object.borrow::<Game>();
            match (&game.capsule, &game.cover) {
                (Some(poster), _) => {
                    art.set_content_fit(gtk::ContentFit::Cover);
                    art.set(Some(poster), &game.initial(), None);
                }
                (None, header) => {
                    art.set_content_fit(gtk::ContentFit::Contain);
                    art.set(header.as_deref(), &game.initial(), Some(CARD_WIDTH));
                }
            }
            name.set_text(&game.name);
            name.set_tooltip_text(Some(&game.name));
            caption.set_text(&game.card_caption());
            for class in ["state-ready", "state-planned", "state-warning", "state-disabled"] {
                caption.remove_css_class(class);
            }
            caption.add_css_class(game.state.dot_class().replace("dot-", "state-").as_str());
        });
    });
    factory
}
