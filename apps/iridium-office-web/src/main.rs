//! Workspace in the browser: the desktop stub.
//!
//! The browser build starts from the library's `#[wasm_bindgen(start)]`
//! function, which wasm-bindgen calls when the module is instantiated. This
//! binary exists because Trunk builds a named binary target; it does NOT
//! start the suite, or the suite would start twice.

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "iridium-office-web only runs in the browser: build it with \
         `trunk build --release` in apps/iridium-office-web"
    );
}
