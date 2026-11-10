# PHASE 1 — PDF (pdfcraft)

1. **VERDICT: SHELL LIBRARY DONE**

2. **Gate:** `cd /opt/hermes-agent/workspace/pdfcraft && export CARGO_BUILD_JOBS=4 CRAFT_FONTS_DIR=/tmp/craft-fonts CRAFT_I18N_DIR=/tmp/craft-i18n PKG_CONFIG_PATH=.../alsa-stub/pkgconfig LIBRARY_PATH=.../alsa-stub/lib; umask 022; cargo xtask ci` → **exit 0**, "CI: all 10 steps passed" (fmt, clippy -D warnings, cli-no-default-features, test, i18n-report, layers, wasm, license-check/assets, deny, parity). `deny` self-SKIPS ("cargo-deny is not installed") — pre-existing, not run.

3. **New crate:** `crates/pdf-shell/`, package `iridium-pdf-shell` 0.18.1 (also added to root `[workspace.dependencies]`). Public API:
```rust
#[derive(Clone, Default)] pub struct Launch { pub files: Vec<String>, pub options: Vec<(String,String)>, pub control_file: Option<String>,
  pub create_images: bool, pub combine: bool, pub integrated: bool,
  pub window_controls: Option<iridium_pdf_ui_egui::window_controls::Layout>, pub two_paged: bool,
  pub os_events: Option<OsEventsConnect>, pub startup_error: Rc<RefCell<Option<String>>> }
pub type OsEventsConnect = Rc<dyn Fn(&egui::Context, bool) -> iridium_pdf_ui_egui::OsEventsFn>;
pub struct Shell { /* PdfApp */ }
impl Shell {
  pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self
  pub fn logic(&mut self, ctx: &egui::Context)
  pub fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput)
  pub fn ui(&mut self, ui: &mut egui::Ui)
  pub fn open_path(&mut self, path: &str)
  pub fn save_state(&mut self)
  pub fn can_close(&self) -> bool
}
impl eframe::App for Shell   // extra, see §6
```
Shell WRAPS `PdfApp`. `can_close` = `!PdfApp::work_at_risk()` (existing: any doc with unsaved work incl. uncommitted field typing, or an open OCR review — the same test `guard_quit` uses). I made that method `pub` (was `pub(crate)`). I also added `PdfApp::run_logic(ctx)` / `run_ui(ui)` (frame-less bodies of the eframe::App `logic`/`ui`, which now delegate to them) in `crates/ui-egui/src/app.rs`, because the required API has no `eframe::Frame`. Zero new external deps (raw-window-handle, windows, winit pins copied from the app's own manifest).

4. **Moved to shell:** the whole `app_creator` closure body (units, theme, restore incl. legacy "printcraft" key, window state, text scale, reopen pref, Windows taskbar icon + motion source, startup update check, printers, macOS native menu, control channel, recovery, OCR dictionary dir, wgpu adapter log, software-renderer notice, portable warning, file/combine/create-images opening, options), plus `open_control_channel`/`write_control_file`/`stage_control_file` (+ their 6 tests), `notify_software_renderer` (+test), `native_wayland`, taskbar-icon fns, `startup::open_files` (+3 tests), `reopen.rs`, `motion.rs` (git mv), `text_scale.rs` (copy; main keeps only its `gsettings` helper for `decorations.rs`).
   **Stayed in main.rs:** panic hook + `logging::install` (log::set_logger), arg parsing (`startup::Arguments`, `path_from_arg`, `rejoin_split_paths`), `--help/--version`, legacy-folder migration, `settings_dir`, single-instance election (`single_instance.rs`), `APP_ID`/icon + `native_options`, GPU config (`configure_gpu`, adapter picking, limits), wgpu→GL retry, Apple-events install (macOS), `decorations`, and the Windows `craft_update::self_update::finish` call. main.rs is now: parse → logging → claim instance → `Launch` → `eframe::run_native(.., app_creator)` where the creator is `Shell::new` + fail on `startup_error`.
   **No macOS `process::exit(0)` exists in this repo** (`PdfApp::on_exit` only calls `updates.exiting()`), so nothing to keep; `UPDATE_APP` stays in ui-egui and main only calls `finish`.
   **gates.rs:656:** it is a unit test asserting `const APP_ID: &str = "ph.pdf"` in main.rs; `APP_ID` stayed in main.rs, so it holds unchanged — no gate edit. **Other main-reading gate found by running it:** parity (`parity/acrobat-features.toml`) cited `apps/iridium-pdf/src/startup.rs::startup_override_is_per_file_and_does_not_persist`; that test moved, so I repointed the evidence path to `crates/pdf-shell/src/startup.rs::…`.
   Layers: `("shell", Class::Exempt)` in `xtask/src/layers.rs` (short name of `iridium-pdf-shell` is `shell`). Exempt rather than L7 because the wasm gate checks exactly the `Layer`/`Standalone` classes, so Exempt is what keeps this desktop-only crate out of wasm (same as the app); an L7 entry would have pulled it into the wasm build and needed a SIDEWAYS edge to ui-egui. Assets/ATTRIBUTION/LICENSES: no covered file moved (icons stay in assets/; shell `include_bytes!`s the Windows one) — assets step ok, no hash change.

5. **Obstacles:** (a) `error[E0425]: cannot find function write_control_file in module super` (apps/iridium-pdf/src/tests.rs ×3) → moved those 5 tests + `scratch`/`written` helpers into `pdf-shell/src/control.rs`. (b) f-string brace NameError in my generator script → rewrote via a template file. (c) `parity error: view.layout-two-up: evidence: no fn startup_override_is_per_file_and_does_not_persist in apps/iridium-pdf/src/startup.rs` → repointed (above). (d) Lifetime problem: old creator borrowed `&Server`/`&AppleEvents`; solved by moving `Server` (it only holds an Arc) / an `Rc<AppleEvents>` into an `OsEventsConnect` closure carried in `Launch`.

6. **Not done / deviations (honest):**
   - `--control` failure used to return `Err` from the app creator ("the app doesn't start"). `Shell::new` must return `Self`, so it records the message in `Launch.startup_error` (shared `Rc`) and returns early before any file opens; main's creator turns it into the same `Err`. Same behaviour, but a host must check that cell.
   - `save_state()` has no storage handle by the required signature, so it only runs the on-quit hook (`on_exit`: start a pending update). Preferences/window state persist through `eframe::App::save(storage)`, which `Shell` implements (extra trait impl beyond the listed methods; also how main hosts it). A suite host must forward `save` to it.
   - macOS ordering nit: `os_events` is now set after the native menu install instead of before (independent). macOS and Windows code paths were only type-reviewed — compiled/tested on Linux only. Windows manifest: removed now-unused `windows`/`winit` target deps from the app (they moved to the shell).
   - Not launched interactively (no display): behaviour parity rests on the unchanged moved code + full test suite.

7. **git status --porcelain:**
```
 M Cargo.lock
 M Cargo.toml
 M apps/iridium-pdf/Cargo.toml
 M apps/iridium-pdf/src/main.rs
 D apps/iridium-pdf/src/motion.rs
 D apps/iridium-pdf/src/reopen.rs
 M apps/iridium-pdf/src/startup.rs
 M apps/iridium-pdf/src/tests.rs
 M apps/iridium-pdf/src/text_scale.rs
A  crates/pdf-shell/src/motion.rs
A  crates/pdf-shell/src/reopen.rs
 M crates/ui-egui/src/app.rs
 M crates/ui-egui/src/editing/close.rs
 M parity/acrobat-features.toml
 M xtask/src/layers.rs
?? crates/pdf-shell/Cargo.toml
?? crates/pdf-shell/src/control.rs
?? crates/pdf-shell/src/lib.rs
?? crates/pdf-shell/src/startup.rs
?? crates/pdf-shell/src/text_scale.rs
```
