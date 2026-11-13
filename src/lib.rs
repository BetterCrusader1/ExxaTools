//! ExxaTools library: every module the binary and the tests use.

pub mod appmanifest;
pub mod steamui;
pub mod window;
pub mod desktop;
pub mod cdp;
pub mod diagnose;
pub mod diagnostics;
pub mod doctor;
pub mod engine;
pub mod http;
pub mod lua;
pub mod paths;
pub mod pins;
pub mod plans;
pub mod plugins;
pub mod providers;
pub mod selftest;
pub mod server;
pub mod sources;
pub mod verify;
pub mod state;
pub mod storage;
pub mod steam;
pub mod hubcap;
pub mod steamctl;
pub mod vfsys;
pub mod watch;
pub mod ui;
pub mod vdf;

pub fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
