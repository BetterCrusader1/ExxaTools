//! Compatibility entry point: the native dashboard lives in `crate::ui`.

pub fn run(url: &str) -> Result<(), String> {
    crate::ui::run(url)
}
