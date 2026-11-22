//! System explains the machine in plain language, with the technical detail one layer down.

use std::time::Duration;

use adw::prelude::*;
use gtk::{Box as GtkBox, Orientation};

use crate::ui::model::{DoctorItem, DoctorReport};
use crate::ui::shell::Ctx;
use crate::ui::worker::Command;

const GROUPS: [&str; 6] = ["Engine", "Steam", "Plans", "Storage", "Sources", "Other"];

pub struct SystemPage {
    pub root: gtk::Widget,
    summary_icon: gtk::Image,
    summary_spinner: gtk::Spinner,
    summary_title: gtk::Label,
    summary_detail: gtk::Label,
    body: gtk::Box,
    ctx: Ctx,
}

struct SecondaryButton {
    root: gtk::Button,
    icon: gtk::Image,
    label: gtk::Label,
}

pub fn build(ctx: &Ctx) -> SystemPage {
    let title = gtk::Label::new(Some("System"));
    title.add_css_class("title-1");
    title.set_halign(gtk::Align::Start);
    let subtitle = gtk::Label::new(Some("Runtime checks and integration status"));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk::Align::Start);
    subtitle.set_xalign(0.0);
    subtitle.set_wrap(true);
    let headings = GtkBox::new(Orientation::Vertical, 2);
    headings.set_hexpand(true);
    headings.append(&title);
    headings.append(&subtitle);

    let copy = secondary_button("Copy report", "edit-copy-symbolic");
    copy.root
        .set_tooltip_text(Some("Copy the full diagnostic report"));
    let commands = ctx.commands.clone();
    let copy_icon = copy.icon.clone();
    let copy_label = copy.label.clone();
    copy.root.connect_clicked(move |_| {
        commands.send(Command::CopyDiagnostics);
        copy_icon.set_icon_name(Some("emblem-ok-symbolic"));
        copy_icon.add_css_class("success");
        copy_label.set_text("Copied");
        let icon = copy_icon.clone();
        let label = copy_label.clone();
        gtk::glib::timeout_add_local_once(Duration::from_millis(1400), move || {
            icon.set_icon_name(Some("edit-copy-symbolic"));
            icon.remove_css_class("success");
            label.set_text("Copy report");
        });
    });

    let header = GtkBox::new(Orientation::Horizontal, 8);
    header.set_margin_top(22);
    header.set_margin_bottom(8);
    header.set_margin_start(24);
    header.set_margin_end(24);
    header.append(&headings);
    header.append(&copy.root);

    let summary_icon = gtk::Image::from_icon_name("emblem-ok-symbolic");
    summary_icon.set_pixel_size(28);
    summary_icon.set_valign(gtk::Align::Center);
    let summary_spinner = gtk::Spinner::new();
    summary_spinner.set_size_request(28, 28);
    summary_spinner.set_visible(false);
    let summary_visual = GtkBox::new(Orientation::Horizontal, 0);
    summary_visual.set_size_request(28, 28);
    summary_visual.set_valign(gtk::Align::Center);
    summary_visual.append(&summary_icon);
    summary_visual.append(&summary_spinner);
    let summary_title = gtk::Label::new(Some("Checking the setup"));
    summary_title.add_css_class("heading");
    summary_title.set_halign(gtk::Align::Start);
    summary_title.set_xalign(0.0);
    summary_title.set_wrap(true);
    let summary_detail = gtk::Label::new(Some("Reading engine, Steam and plan state."));
    summary_detail.add_css_class("dim-label");
    summary_detail.set_halign(gtk::Align::Start);
    summary_detail.set_xalign(0.0);
    summary_detail.set_wrap(true);
    let summary_text = GtkBox::new(Orientation::Vertical, 2);
    summary_text.set_hexpand(true);
    summary_text.append(&summary_title);
    summary_text.append(&summary_detail);

    let summary = GtkBox::new(Orientation::Horizontal, 14);
    summary.set_margin_start(24);
    summary.set_margin_end(24);
    summary.set_margin_top(8);
    summary.set_margin_bottom(8);
    summary.append(&summary_visual);
    summary.append(&summary_text);

    let body = GtkBox::new(Orientation::Vertical, 12);
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
        summary_spinner,
        summary_title,
        summary_detail,
        body,
        ctx: ctx.clone(),
    }
}

impl SystemPage {
    pub fn set_report(&self, report: &DoctorReport) {
        self.summary_spinner.stop();
        self.summary_spinner.set_visible(false);
        self.summary_icon.set_visible(true);
        clear_summary_status(&self.summary_icon);
        let (icon, title) = report.summary();
        self.summary_icon.set_icon_name(Some(icon));
        self.summary_icon
            .add_css_class(summary_status_class(report));
        self.summary_title.set_text(title);
        let failures = report.failed();
        let where_ = report
            .items
            .iter()
            .find(|item| item.status != "ok")
            .map(|item| format!(" ({})", item.group()))
            .unwrap_or_default();
        self.summary_detail.set_text(&format!(
            "{} checks, {} {}{}, {} {}",
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
                group.add(&doctor_row(item, &self.ctx));
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
            let group = adw::PreferencesGroup::builder()
                .title(group_name)
                .description(group_description(group_name))
                .build();
            for item in items {
                group.add(&doctor_row(item, &self.ctx));
            }
            self.body.append(&group);
        }
    }

    pub fn show_loading(&self) {
        show_summary_loading(
            &self.summary_icon,
            &self.summary_spinner,
            &self.summary_title,
            &self.summary_detail,
        );
    }
}

fn show_summary_loading(
    icon: &gtk::Image,
    spinner: &gtk::Spinner,
    title: &gtk::Label,
    detail: &gtk::Label,
) {
    clear_summary_status(icon);
    icon.add_css_class("dim-label");
    icon.set_icon_name(Some("content-loading-symbolic"));
    icon.set_visible(false);
    spinner.set_visible(true);
    spinner.start();
    title.set_text("Checking the setup");
    detail.set_text("Reading engine, Steam and plan state.");
}

fn secondary_button(label: &str, icon_name: &str) -> SecondaryButton {
    let root = gtk::Button::new();
    root.set_valign(gtk::Align::Center);
    root.set_cursor_from_name(Some("pointer"));
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(16);
    let label = gtk::Label::new(Some(label));
    let content = GtkBox::new(Orientation::Horizontal, 8);
    content.append(&icon);
    content.append(&label);
    root.set_child(Some(&content));
    SecondaryButton { root, icon, label }
}

fn clear_summary_status(icon: &gtk::Image) {
    for class in ["success", "warning", "error", "dim-label"] {
        icon.remove_css_class(class);
    }
}

fn summary_status_class(report: &DoctorReport) -> &'static str {
    if report.failed() > 0 {
        "error"
    } else if report.warnings > 0 || report.items.iter().any(|item| item.status == "warn") {
        "warning"
    } else {
        "success"
    }
}

fn group_description(name: &str) -> &'static str {
    match name {
        "Engine" => "Vapor Forge installation and client integration",
        "Steam" => "Client startup and debugging integration",
        "Plans" => "Plan files, manifest provider and depot keys",
        "Storage" => "Library capacity and install state",
        "Sources" => "Optional manifest and key sources",
        _ => "Additional runtime checks",
    }
}

fn doctor_row(item: &DoctorItem, ctx: &Ctx) -> gtk::Widget {
    let subtitle = row_subtitle(item);
    let has_detail = !item.detail.trim().is_empty();
    let expandable = has_detail
        && (technical_value(item).is_some()
            || (item.status != "ok" && !item.hint.trim().is_empty()));

    if expandable {
        let row = adw::ExpanderRow::builder()
            .title(&item.label)
            .subtitle(&subtitle)
            .build();
        row.set_subtitle_lines(3);
        row.add_prefix(&status_led(item));
        if item.status != "ok" {
            row.add_suffix(&status_badge(item));
        }
        row.add_row(&detail_row(item, ctx));
        row.upcast()
    } else {
        let row = adw::ActionRow::builder()
            .title(&item.label)
            .subtitle(&subtitle)
            .build();
        row.set_subtitle_lines(3);
        row.add_prefix(&status_led(item));
        if item.status != "ok" {
            row.add_suffix(&status_badge(item));
        }
        row.upcast()
    }
}

fn row_subtitle(item: &DoctorItem) -> String {
    if item.status != "ok" && !item.hint.trim().is_empty() {
        return item.hint.clone();
    }
    if technical_value(item).is_some() {
        return technical_summary(item).to_string();
    }
    if !item.detail.trim().is_empty() {
        return item.detail.clone();
    }
    item.hint.clone()
}

fn technical_summary(item: &DoctorItem) -> &'static str {
    match item.id.as_str() {
        "engine" => "Installed version",
        "ld_audit" => "Launcher library path resolved",
        "scripts" => "Directory is writable",
        "provider" => "Provider location available",
        "debug_port" => "Open on this machine",
        "keys" => "Missing depot identifiers",
        _ => "Technical detail available",
    }
}

fn technical_value(item: &DoctorItem) -> Option<&str> {
    let value = item.detail.trim();
    if value.is_empty() {
        return None;
    }
    match item.id.as_str() {
        "engine" | "ld_audit" | "scripts" | "provider" | "debug_port" => Some(value),
        "keys" if value.starts_with("missing:") => Some(value),
        _ => None,
    }
}

fn status_led(item: &DoctorItem) -> GtkBox {
    let (class, status) = match item.status.as_str() {
        "ok" => ("dot-ready", "Healthy"),
        "bad" => ("dot-warning", "Error"),
        _ => ("dot-planned", "Needs attention"),
    };
    let led = GtkBox::new(Orientation::Horizontal, 0);
    led.add_css_class("status-dot");
    led.add_css_class(class);
    led.set_size_request(10, 10);
    led.set_valign(gtk::Align::Center);
    led.set_tooltip_text(Some(status));
    led
}

fn status_badge(item: &DoctorItem) -> gtk::Label {
    let (text, class) = if item.status == "bad" {
        ("Error", "pill-warning")
    } else {
        ("Warning", "pill-planned")
    };
    let badge = gtk::Label::new(Some(text));
    badge.add_css_class("status-pill");
    badge.add_css_class(class);
    badge.set_valign(gtk::Align::Center);
    badge
}

fn detail_row(item: &DoctorItem, ctx: &Ctx) -> GtkBox {
    let technical = technical_value(item);
    let heading = gtk::Label::new(Some(if technical.is_some() {
        "Technical value"
    } else {
        "Check result"
    }));
    heading.add_css_class("caption");
    heading.add_css_class("dim-label");
    heading.set_halign(gtk::Align::Start);
    heading.set_hexpand(true);

    let header = GtkBox::new(Orientation::Horizontal, 8);
    header.append(&heading);
    if let Some(value) = technical {
        header.append(&copy_value_button(ctx, &item.label, value));
    }

    let value = gtk::Label::new(Some(item.detail.trim()));
    value.set_halign(gtk::Align::Start);
    value.set_xalign(0.0);
    value.set_wrap(true);
    value.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    value.set_max_width_chars(80);
    value.set_selectable(true);
    value.set_tooltip_text(Some(item.detail.trim()));
    if technical.is_some() {
        value.add_css_class("mono");
    }

    let detail = GtkBox::new(Orientation::Vertical, 6);
    detail.set_margin_top(8);
    detail.set_margin_bottom(12);
    detail.set_margin_start(16);
    detail.set_margin_end(12);
    detail.append(&header);
    detail.append(&value);
    detail
}

fn copy_value_button(ctx: &Ctx, label: &str, value: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name("edit-copy-symbolic");
    button.add_css_class("flat");
    button.set_size_request(32, 32);
    button.set_valign(gtk::Align::Center);
    button.set_cursor_from_name(Some("pointer"));
    let tooltip = format!("Copy {label}");
    button.set_tooltip_text(Some(&tooltip));

    let ctx = ctx.clone();
    let value = value.to_string();
    let confirmation = format!("{label} copied");
    let feedback = button.downgrade();
    button.connect_clicked(move |_| {
        ctx.clipboard(&value);
        ctx.toast_text(&confirmation);
        let Some(button) = feedback.upgrade() else {
            return;
        };
        button.set_icon_name("emblem-ok-symbolic");
        button.add_css_class("success");
        button.set_tooltip_text(Some("Copied"));
        let feedback = button.downgrade();
        let tooltip = tooltip.clone();
        gtk::glib::timeout_add_local_once(Duration::from_millis(1400), move || {
            let Some(button) = feedback.upgrade() else {
                return;
            };
            button.set_icon_name("edit-copy-symbolic");
            button.remove_css_class("success");
            button.set_tooltip_text(Some(&tooltip));
        });
    });
    button
}
