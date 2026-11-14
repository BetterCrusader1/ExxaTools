//! Discover is a deliberate browsing surface, not the library with different data.

use adw::prelude::*;
use gtk::{Box as GtkBox, Orientation};

use crate::ui::model::Game;
use crate::ui::shelf::monogram;
use crate::ui::shell::Ctx;
use crate::ui::worker::Command;

pub struct DiscoverPage {
    pub root: gtk::Widget,
    stack: gtk::Stack,
    flow: gtk::FlowBox,
    error: adw::StatusPage,
    loading: adw::StatusPage,
    ctx: Ctx,
}

pub fn build(ctx: &Ctx) -> DiscoverPage {
    let title = gtk::Label::new(Some("Discover"));
    title.add_css_class("page-title");
    title.set_halign(gtk::Align::Start);

    let subtitle = gtk::Label::new(Some("Popular on Steam right now"));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);

    let headings = GtkBox::new(Orientation::Vertical, 2);
    headings.set_hexpand(true);
    headings.append(&title);
    headings.append(&subtitle);

    let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh.add_css_class("flat");
    refresh.set_tooltip_text(Some("Refresh Discover"));
    refresh.set_valign(gtk::Align::Center);
    let commands = ctx.commands.clone();
    refresh.connect_clicked(move |_| commands.send(Command::LoadDiscover));

    let header = GtkBox::new(Orientation::Horizontal, 12);
    header.set_margin_top(22);
    header.set_margin_bottom(8);
    header.set_margin_start(24);
    header.set_margin_end(24);
    header.append(&headings);
    header.append(&refresh);

    let flow = gtk::FlowBox::new();
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_homogeneous(true);
    flow.set_min_children_per_line(2);
    flow.set_max_children_per_line(4);
    flow.set_column_spacing(16);
    flow.set_row_spacing(18);
    flow.set_valign(gtk::Align::Start);
    flow.set_margin_top(12);
    flow.set_margin_bottom(28);
    flow.set_margin_start(24);
    flow.set_margin_end(24);

    let flow_scroll = gtk::ScrolledWindow::new();
    flow_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    flow_scroll.set_child(Some(&flow));
    flow_scroll.set_vexpand(true);

    let loading_spinner = gtk::Spinner::new();
    loading_spinner.set_size_request(28, 28);
    loading_spinner.start();
    let loading = adw::StatusPage::builder()
        .icon_name("system-search-symbolic")
        .title("Looking for something good")
        .description("Reading Steam's current popular titles.")
        .vexpand(true)
        .build();
    loading.set_child(Some(&loading_spinner));

    let error = adw::StatusPage::builder()
        .icon_name("network-error-symbolic")
        .title("Discover is unavailable")
        .description("Steam did not answer. Check the connection and try again.")
        .vexpand(true)
        .build();
    let retry = gtk::Button::with_label("Try again");
    retry.add_css_class("suggested-action");
    retry.set_cursor_from_name(Some("pointer"));
    retry.set_halign(gtk::Align::Center);
    let commands = ctx.commands.clone();
    retry.connect_clicked(move |_| commands.send(Command::LoadDiscover));
    error.set_child(Some(&retry));

    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    stack.add_named(&loading, Some("loading"));
    stack.add_named(&flow_scroll, Some("flow"));
    stack.add_named(&error, Some("error"));
    stack.set_visible_child_name("loading");

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.append(&header);
    root.append(&stack);

    DiscoverPage {
        root: root.upcast(),
        stack,
        flow,
        error,
        loading,
        ctx: ctx.clone(),
    }
}

impl DiscoverPage {
    pub fn show_loading(&self) {
        self.loading
            .set_description(Some("Reading Steam's current popular titles."));
        self.stack.set_visible_child_name("loading");
    }

    pub fn show_error(&self, message: &str) {
        self.error.set_description(Some(message));
        self.stack.set_visible_child_name("error");
    }

    pub fn set_games(&self, games: &[Game]) {
        while let Some(child) = self.flow.first_child() {
            self.flow.remove(&child);
        }
        for game in games {
            self.flow.append(&card(&self.ctx, game));
        }
        self.stack
            .set_visible_child_name(if games.is_empty() { "error" } else { "flow" });
    }
}

fn card(ctx: &Ctx, game: &Game) -> gtk::FlowBoxChild {
    let button = gtk::Button::new();
    button.add_css_class("discover-card");
    button.set_cursor_from_name(Some("pointer"));
    button.set_size_request(232, -1);
    button.set_valign(gtk::Align::Start);

    let cover = match &game.cover {
        Some(path) => match gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, 232, 109, false) {
            Ok(pixbuf) => {
                let picture = gtk::Picture::for_pixbuf(&pixbuf);
                picture.set_content_fit(gtk::ContentFit::Cover);
                picture.set_size_request(232, 109);
                picture.add_css_class("discover-cover");
                picture.upcast::<gtk::Widget>()
            }
            Err(_) => monogram(game, 232, 109),
        },
        None => monogram(game, 232, 109),
    };

    let title = gtk::Label::new(Some(&game.name));
    title.add_css_class("discover-title");
    title.set_halign(gtk::Align::Start);
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.set_max_width_chars(28);
    title.set_margin_top(8);
    title.set_margin_start(2);
    title.set_margin_bottom(8);

    let content = GtkBox::new(Orientation::Vertical, 0);
    content.set_hexpand(true);
    content.append(&cover);
    content.append(&title);
    button.set_child(Some(&content));

    let ctx = ctx.clone();
    let appid = game.appid;
    let name = game.name.clone();
    button.connect_clicked(move |_| confirm_add(&ctx, appid, &name));

    let child = gtk::FlowBoxChild::new();
    child.set_child(Some(&button));
    child
}

fn confirm_add(ctx: &Ctx, appid: u64, name: &str) {
    let dialog = adw::MessageDialog::new(
        Some(&ctx.window),
        Some("Add to library?"),
        Some(&format!(
            "ExxaTools will build a plan for {name} and pin its manifests so Steam can install it."
        )),
    );
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("add", "Add to library");
    dialog.set_response_appearance("add", adw::ResponseAppearance::Suggested);
    let commands = ctx.commands.clone();
    let name = name.to_string();
    dialog.connect_response(None, move |dialog, response| {
        dialog.close();
        if response == "add" {
            commands.send(Command::Add {
                appid,
                name: name.clone(),
            });
        }
    });
    dialog.present();
}
