//! The Office suite: five apps in one window.
//!
//! The binary in `main.rs` is a thin wrapper over this library.

/// The suite's name, slug and app id, from `branding/brand.env` - the one place they
/// live, so changing one is a one-line edit. It is read at compile time, so the built
/// app does not need the file beside it.
const BRAND: &str = include_str!("../../../branding/brand.env");

/// One value out of the brand file.
pub fn brand(key: &str) -> &'static str {
    BRAND
        .lines()
        .find_map(|line| line.strip_prefix(key).and_then(|l| l.strip_prefix("=\"")))
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or_default()
}

pub mod look;
pub mod module;
pub mod modules;
pub mod panic;
pub mod route;
pub mod suite;
