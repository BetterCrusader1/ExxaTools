//! Native dashboard: a depot desk built from small, testable modules.

pub mod art;
pub mod backdrop;
pub mod collection;
pub mod covers;
pub mod discover;
pub mod inspector;
pub mod model;
pub mod settings;
pub mod shell;
pub mod system;
pub mod theme;
pub mod worker;

pub use shell::run;
