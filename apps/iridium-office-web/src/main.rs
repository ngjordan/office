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

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "iridium-office-web only runs in the browser: build it with \
         `trunk build --release` in apps/iridium-office-web"
    );
}
