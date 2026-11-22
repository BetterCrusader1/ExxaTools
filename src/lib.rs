//! ExxaTools library: every module the binary and the tests use.

pub mod appmanifest;
pub mod cdp;
pub mod desktop;
pub mod diagnose;
pub mod diagnostics;
pub mod doctor;
pub mod engine;
pub mod http;
pub mod live;
pub mod hubcap;
pub mod lua;
pub mod paths;
pub mod pins;
pub mod plans;
pub mod plugins;
pub mod providers;
pub mod selftest;
pub mod server;
pub mod sources;
pub mod state;
pub mod steam;
pub mod steamctl;
pub mod steamui;
pub mod storage;
pub mod ui;
pub mod vdf;
pub mod verify;
pub mod vfsys;
pub mod watch;
pub mod window;

pub fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
