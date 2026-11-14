//! Workspace in the browser.
//!
//! The same suite as the desktop app: nine apps in one window. It runs on
//! eframe's web runner (wgpu: WebGPU where available, WebGL2 otherwise).
//! Build with `trunk build --release` from this directory; the site lands in
//! `dist/web`.
//!
//! What differs from the desktop app, and why:
//!
//! - The embedded browser is not there. It drives an installed Chromium over
//!   the DevTools Protocol, which needs a child process and a WebSocket; a
//!   browser tab cannot spawn either. The rail shows the eight editors.
//! - Files come from the browser's file picker and go back as downloads. The
//!   desktop suite reads and writes the file system directly; here the same
//!   engine sits behind the app's own file service, so the apps that already
//!   have a web build (Docs, Film) keep their behaviour and the others open
//!   an empty document until they are given one.
//! - Preferences live in `localStorage`.

#[cfg(target_arch = "wasm32")]
mod web;

/// The entry point.
///
/// wasm-bindgen runs this when the module is instantiated, which is what
/// starts the suite in the browser. A binary's `main` is a raw export of the
/// wasm module and not a JavaScript binding, so the page cannot reach it;
/// this function can.
///
/// The crate is a cdylib for that reason, and the binary calls this function
/// because a binary target has no start hook of its own.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    web::start();
}
