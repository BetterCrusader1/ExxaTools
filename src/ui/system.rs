//! System explains the machine in plain language, with the technical detail one layer down.

use adw::prelude::*;
use gtk::{Box as GtkBox, Orientation};

use crate::ui::model::DoctorReport;
use crate::ui::shell::Ctx;
use crate::ui::worker::Command;

const GROUPS: [&str; 6] = ["Engine", "Steam", "Plans", "Storage", "Sources", "Other"];

pub struct SystemPage {
    pub root: gtk::Widget,
    summary_icon: gtk::Image,
    summary_title: gtk::Label,
    summary_detail: gtk::Label,
    body: gtk::Box,
    ctx: Ctx,
}

pub fn build(ctx: &Ctx) -> SystemPage {
    let title = gtk::Label::new(Some("System"));
    title.add_css_class("page-title");
    title.set_halign(gtk::Align::Start);
    let subtitle = gtk::Label::new(Some("Runtime checks and integration status"));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);
    let headings = GtkBox::new(Orientation::Vertical, 2);
    headings.set_hexpand(true);
    headings.append(&title);
    headings.append(&subtitle);

    let copy = gtk::Button::with_label("Copy report");
    copy.add_css_class("flat");
    copy.set_valign(gtk::Align::Center);
    let commands = ctx.commands.clone();
    copy.connect_clicked(move |_| commands.send(Command::CopyDiagnostics));

    let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh.add_css_class("flat");
    refresh.set_tooltip_text(Some("Run the checks again"));
    refresh.set_valign(gtk::Align::Center);
    let commands = ctx.commands.clone();
    refresh.connect_clicked(move |_| commands.send(Command::LoadDoctor));

    let header = GtkBox::new(Orientation::Horizontal, 8);
    header.set_margin_top(22);
    header.set_margin_bottom(8);
    header.set_margin_start(24);
    header.set_margin_end(24);
    header.append(&headings);
    header.append(&copy);
    header.append(&refresh);

    let summary_icon = gtk::Image::from_icon_name("emblem-ok-symbolic");
    summary_icon.set_pixel_size(28);
    summary_icon.set_valign(gtk::Align::Center);
    let summary_title = gtk::Label::new(Some("Checking the setup"));
    summary_title.add_css_class("summary-title");
    summary_title.set_halign(gtk::Align::Start);
    let summary_detail = gtk::Label::new(Some("Reading engine, Steam and plan state."));
    summary_detail.add_css_class("dim-label");
    summary_detail.set_halign(gtk::Align::Start);
    let summary_text = GtkBox::new(Orientation::Vertical, 2);
    summary_text.set_hexpand(true);
    summary_text.append(&summary_title);
    summary_text.append(&summary_detail);

    let summary = GtkBox::new(Orientation::Horizontal, 14);
    summary.add_css_class("summary-card");
    summary.set_margin_start(24);
    summary.set_margin_end(24);
    summary.set_margin_top(8);
    summary.set_margin_bottom(8);
    summary.append(&summary_icon);
    summary.append(&summary_text);

    let body = GtkBox::new(Orientation::Vertical, 6);
    body.set_margin_top(8);
    body.set_margin_bottom(28);
    body.set_margin_start(24);
    body.set_margin_end(24);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_child(Some(&body));
    scroll.set_vexpand(true);

    let content = GtkBox::new(Orientation::Vertical, 0);
    content.append(&header);
    content.append(&summary);
    content.append(&scroll);

    let clamp = adw::Clamp::builder()
        .maximum_size(860)
        .vexpand(true)
        .build();
    clamp.set_child(Some(&content));

    SystemPage {
        root: clamp.upcast(),
        summary_icon,
        summary_title,
        summary_detail,
        body,
        ctx: ctx.clone(),
    }
}

impl SystemPage {
    pub fn set_report(&self, report: &DoctorReport) {
        let (icon, title) = report.summary();
        self.summary_icon.set_icon_name(Some(icon));
        self.summary_title.set_text(title);
        let failures = report.failed();
        let where_ = report
            .items
            .iter()
            .find(|item| item.status != "ok")
            .map(|item| format!(" ({})", item.group()))
            .unwrap_or_default();
        self.summary_detail.set_text(&format!(
            "{} checks · {} {}{} · {} {}",
            report.items.len(),
            report.warnings,
            if report.warnings == 1 {
                "warning"
            } else {
                "warnings"
            },
            where_,
            failures,
            if failures == 1 { "failure" } else { "failures" }
        ));

        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }

        let attention: Vec<_> = report
            .items
            .iter()
            .filter(|item| item.status != "ok")
            .collect();
        if !attention.is_empty() {
            let group = adw::PreferencesGroup::builder()
                .title("Needs attention")
                .description("These checks block a clean setup or need a decision.")
                .build();
            for item in attention {
                group.add(&doctor_row(item));
            }
            self.body.append(&group);
        }

        for group_name in GROUPS {
            let items: Vec<_> = report
                .items
                .iter()
                .filter(|item| item.group() == group_name && item.status == "ok")
                .collect();
            if items.is_empty() {
                continue;
            }
            let group = adw::PreferencesGroup::builder().title(group_name).build();
            for item in items {
                group.add(&doctor_row(item));
            }
            self.body.append(&group);
        }
    }

    pub fn show_loading(&self) {
        self.summary_icon
            .set_icon_name(Some("content-loading-symbolic"));
        self.summary_title.set_text("Checking the setup");
        self.summary_detail
            .set_text("Reading engine, Steam and plan state.");
        let _ = &self.ctx;
    }
}

fn doctor_row(item: &crate::ui::model::DoctorItem) -> adw::ActionRow {
    let icon_name = match item.status.as_str() {
        "ok" => "emblem-ok-symbolic",
        "bad" => "dialog-error-symbolic",
        _ => "dialog-warning-symbolic",
    };
    let subtitle = if item.status == "ok" || item.hint.is_empty() {
        item.detail.clone()
    } else {
        format!("{} · {}", item.detail, item.hint)
    };
    let row = adw::ActionRow::builder()
        .title(&item.label)
        .subtitle(&subtitle)
        .build();
    row.set_subtitle_lines(2);
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_valign(gtk::Align::Center);
    row.add_prefix(&icon);
    row
}
