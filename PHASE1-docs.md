# PHASE 1 — Docs (wc-wordcraft)

1. **VERDICT: SHELL LIBRARY DONE** (Linux gate green; macOS/Windows-only code compiles only there and was not built — see §6).

2. **Gate:** `cd /opt/hermes-agent/workspace/wc-wordcraft && export CARGO_BUILD_JOBS=4 CRAFT_FONTS_DIR=/tmp/craft-fonts CRAFT_I18N_DIR=/tmp/craft-i18n && umask 022 && cargo xtask ci` → **exit 0**. "all 9 steps passed (fmt, clippy, test, i18n-report, language, doctest, assets, layers, wasm)". Log: /tmp/docs-shell-ci.log.

3. **Crate:** `crates/docs-shell/`, package `iridium-docs-shell` (lib `iridium_docs_shell`), root `#![deny(clippy::unwrap_used, …)]`, no new external deps. Public API:
```rust
pub struct Launch {
    pub files: Vec<String>,
    pub other_files: Vec<std::path::PathBuf>,   // non-UTF-8 names (#1440)
    pub sample: bool,
    pub control_port: Option<u16>,
    pub window: Option<WindowGeometry>,          // restored geometry
    pub window_builder: egui::ViewportBuilder,
    #[cfg(target_os = "macos")] pub apple_events: fmv_macos_events::Inbox,
}
pub struct Shell { /* private */ }
impl Shell {
    pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self;
    pub fn logic(&mut self, ctx: &egui::Context);
    pub fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput);
    pub fn ui(&mut self, ui: &mut egui::Ui);
    pub fn open_path(&mut self, path: &str);
    pub fn save_state(&mut self);     // finish_pending_jobs + finish_recovery + save_prefs + remove control key file
    pub fn can_close(&self) -> bool;  // !printing.busy() && !has_unsaved_documents()
}
```
**Additions beyond the brief's surface (needed to keep behaviour identical):**
- `Shell::show_file_dialogs(&self, frame: &eframe::Frame)` — rfd dialogs are parented to `eframe::Frame`, which `logic`/`ui` don't receive. Host must call it after `logic` and `ui` (Phase 2 must supply a Frame, or accept unparented dialogs).
- `#[cfg(macos)] Shell::take_quit(&mut self) -> bool` — shell decides a close is allowed; host does the process quit.
- `#[cfg(macos)] pub mod apple_events` (`AppleEvents::install/connect`, `poll`) — the handler is registered process-wide in main; the shell only drains the inbox.
- Free fns for pre-window work in main: `pub fn log_dir()`, `prefs_enabled()`, `load_user_fonts()`, `saved_window()`.
- `can_close` required `WordApp::has_unsaved_documents` to go `pub(crate)` → `pub` (crates/ui-egui/src/documents.rs, one word). `logic` still gates closes via the existing `close_requested()` (which prompts); `can_close` is a non-prompting query.

4. **Moved to the shell** (`git mv`, tests travel): `animations`, `apple_events`, `clipboard`, `control_server`, `default_apps`, `file_dialogs`, `print`, `region`, `traffic_lights`; plus from main.rs the whole eframe `App` body (logic/raw_input_hook/ui/on_exit minus macOS terminate), `services()`, prefs load/save/`write_prefs`, user dictionary, `open_non_unicode`, the creator-closure setup, and tests `a_file_whose_name_isnt_unicode…`, `a_pipe_or_device…`, `failed_preferences_write…`. Platform deps (arboard, rfd, craft-print, zbus, wl-clipboard-rs, windows, clipboard-win, open, getrandom, fmv-macos-events, objc2*) moved to the shell Cargo.toml.
   **Stayed in main.rs:** `text_args`/`without_launch_args`/USAGE/`--version` parsing (+ its tests); `logging` (`log::set_logger`, panic hook); `graphics` (cfg windows/macos/test, `configure`, `configure_indirect_validation`); `app_icon` + `NativeOptions`/viewport/X11 builder; `AppleEvents::install` (process-wide); `schedule_macos_terminate` with `std::process::exit(0)` (calls `shell.save_state()` first); the Windows `craft_update::self_update::finish(&iridium_docs_ui_egui::UPDATE_APP, …)`. main is now a thin `struct App(Shell)` implementing `eframe::App` + `run_native`.
   Manifest/gate edits: root `Cargo.toml` (workspace dep `iridium-docs-shell`; member via `crates/*` glob), `xtask/src/layers.rs` (`("shell", Class::Exempt)`, placed after `("iridium-docs", Class::Exempt)`; Exempt ⇒ not selected by `wasm_set()`, which only takes Layer≤5, ui-egui, Standalone). `xtask` has no gates.rs and no gate reads main.rs. `apps/iridium-docs/Cargo.toml` trimmed to what main still uses.

5. **Obstacles:** first clippy run failed — `error: function `choose_backends` is never used … graphics.rs` (+ 4 more dead-code): I had dropped the `#[cfg(any(target_os = "windows", target_os = "macos", test))]` on `mod graphics;`. Restored it; gate then passed. (Plus the harness rejecting `&` backgrounding; used background mode.)

6. **Not done / caveats:**
   - macOS and Windows code paths (apple_events wiring, `take_quit`, traffic lights, craft-update finish) were not compiled here (Linux host). The macOS quit sequence is now split across shell (`logic` sets pending, sends CancelClose, clears `quit_requested`) and main (`schedule_macos_terminate`) with the same order of effects; needs a macOS build to confirm.
   - `Shell::logic` still sends `ViewportCommand::Close` on non-macOS quit (as before); in a suite that targets the host's root viewport — Phase 2 concern.
   - No live GUI launch was run; behaviour parity rests on code moved verbatim + the passing test suite.

7. **git status --porcelain:**
```
 M Cargo.lock
 M Cargo.toml
 M apps/iridium-docs/Cargo.toml
 M apps/iridium-docs/src/main.rs
R  apps/iridium-docs/src/animations.rs -> crates/docs-shell/src/animations.rs
R  apps/iridium-docs/src/animations/desktop.rs -> crates/docs-shell/src/animations/desktop.rs
R  apps/iridium-docs/src/apple_events.rs -> crates/docs-shell/src/apple_events.rs
R  apps/iridium-docs/src/clipboard.rs -> crates/docs-shell/src/clipboard.rs
R  apps/iridium-docs/src/control_server.rs -> crates/docs-shell/src/control_server.rs
R  apps/iridium-docs/src/default_apps.rs -> crates/docs-shell/src/default_apps.rs
R  apps/iridium-docs/src/file_dialogs.rs -> crates/docs-shell/src/file_dialogs.rs
R  apps/iridium-docs/src/print.rs -> crates/docs-shell/src/print.rs
R  apps/iridium-docs/src/region.rs -> crates/docs-shell/src/region.rs
R  apps/iridium-docs/src/traffic_lights.rs -> crates/docs-shell/src/traffic_lights.rs
 M crates/ui-egui/src/documents.rs
 M xtask/src/layers.rs
?? crates/docs-shell/Cargo.toml
?? crates/docs-shell/src/lib.rs
```
(Renames are staged by `git mv`; nothing committed.)
