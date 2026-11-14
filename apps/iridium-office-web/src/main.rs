//! Workspace in the browser: the desktop stub.
//!
//! The browser build is the library's `#[wasm_bindgen(start)]` function,
//! which wasm-bindgen calls when the module is instantiated. This binary
//! exists because Trunk builds a named binary target, and it calls the same
//! entry point so there is one place that starts the suite.

#[cfg(target_arch = "wasm32")]
fn main() {
    iridium_office_web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "iridium-office-web only runs in the browser: build it with \
         `trunk build --release` in apps/iridium-office-web"
    );
}
