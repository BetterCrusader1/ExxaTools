//! The desktop window: GTK with a WebKit view, so the dashboard is an
//! application on the desktop instead of a tab in whatever browser is around.
//!
//! The renderer runs without GPU acceleration on purpose: this stack cannot
//! allocate a GBM buffer for the surface, and with acceleration on the window
//! stays grey while the page loads behind it.

use gtk::prelude::*;
use webkit2gtk::{SettingsExt, WebViewExt};

pub fn run(url: &str) -> Result<(), String> {
    std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    gtk::init().map_err(|error| format!("cannot start GTK: {error}"))?;

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("ExxaTools");
    window.set_default_size(1320, 880);
    window.set_position(gtk::WindowPosition::Center);
    let icon = crate::paths::web_dir().join("logo-128.png");
    if icon.is_file() {
        let _ = window.set_icon_from_file(&icon);
    }
    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        gtk::glib::Propagation::Proceed
    });

    let view = webkit2gtk::WebView::new();
    // A native select opens a GTK menu, and GTK ignores the page palette. Asking
    // for a dark theme keeps that popup from turning white on a dark dashboard.
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(true);
    }
    let settings = WebViewExt::settings(&view);
    if let Some(settings) = settings {
        settings.set_hardware_acceleration_policy(webkit2gtk::HardwareAccelerationPolicy::Never);
        settings.set_enable_developer_extras(true);
    }
    view.load_uri(url);
    window.add(&view);
    window.show_all();
    window.present();

    gtk::main();
    Ok(())
}
