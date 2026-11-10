# PHASE 1 — Sheets (gc-gridcraft)

1. **VERDICT: SHELL LIBRARY DONE** (Linux only; macOS/Windows cfg code compiled by inspection, not built — see §6)

2. **Gate:** `cd /opt/hermes-agent/workspace/gc-gridcraft && export CARGO_BUILD_JOBS=4 CRAFT_FONTS_DIR=/tmp/craft-fonts CRAFT_I18N_DIR=/tmp/craft-i18n; umask 022; cargo xtask ci` → **exit 0** ("all 10 steps passed: i18n-report, fmt, clippy, test, doctest, assets, layers, wasm, parity, keytips"). wasm set unchanged (L≤5 + sheets-web), so the shell crate is excluded by the existing layer-based selection. Log: /tmp/sheets-ci.log.

3. **Crate:** `crates/sheets-shell/`, package `iridium-sheets-shell` (lib, L7 in `xtask/src/layers.rs` as `("sheets-shell", Class::Layer(7))`; `[workspace.dependencies]` entry added; picked up by `crates/*` members). Root has the `#![deny(clippy::unwrap_used, …)]` attribute.
```rust
#[derive(Clone)]
pub struct Launch {
    pub files: Vec<String>,
    pub sample: Option<String>,
    pub control_port: Option<u16>,
    pub control_token: Option<String>,
    pub viewport: egui::ViewportBuilder,
    pub prefs: bool,                              // false under IRIDIUM_NO_PREFS
    pub config_dir: Option<std::path::PathBuf>,   // settings folder (ui.json, prefs.json)
    #[cfg(target_os = "macos")] pub inbox: fmv_macos_events::Inbox,
}
pub struct Shell { /* private */ }
impl Shell {
    pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self;
    pub fn logic(&mut self, ctx: &egui::Context);
    pub fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput);
    pub fn ui(&mut self, ui: &mut egui::Ui);
    pub fn open_path(&mut self, path: &str);
    pub fn save_state(&mut self);
    pub fn can_close(&self) -> bool;  // !printing.busy() && no DocState::is_dirty()
}
```
No traits, no new external deps (deps copied from the app's Cargo.toml).

4. **Moved to the lib** (`git mv`): `apple_events.rs` (minus the `AppleEvents` struct), `connector.rs`, `control_server.rs`, `gsheets.rs`, `native_menu.rs`, `print.rs`, `tests/mac_menu_appkit.rs` (+ `[[test]] harness=false` and macOS deps); from main.rs: `services()`, clipboard, `open_with_system`, `load_prefs`/`save_prefs`, session/sample/files setup, control-server start, `DocWindows`, native menu + Apple-event polling, dropped-file handling, the print-busy `CancelClose`. `on_exit`'s `finish_pending_jobs` + `save_prefs` = `Shell::save_state`.
   **Stayed in main.rs:** panic hook + `logging` (`log::set_logger`); `graphics.rs` and all backend config (incl. Windows DX12 retry); `config_dir`/`prefs_enabled`/`log_dir` (log-file location; the values are passed to the shell via `Launch`); `icon()`/viewport; arg parsing, `--version`; `craft_update::self_update::finish(&UPDATE_APP…)`; macOS `Registration::install()` (before the event loop; its inbox goes in `Launch`); macOS `std::process::exit(0)` in `on_exit` with its #117 comment; `packaging_tests`. `gates.rs` has no main.rs assertion (grepped).

5. **Obstacles:** `cannot find module or crate iridium_engine` in main.rs/logging.rs → kept `iridium-engine` and `chrono` deps in the app. Dead-code warnings in graphics.rs → restored its `#[cfg(any(windows, macos, test))]` on `mod graphics`. Unused `window_builder` → removed.

6. **Not done / behaviour deltas (be aware):**
   - macOS and Windows paths could not be compiled here (Linux host). The macOS `inbox`/`set_wake` wiring and the Windows retry logic are by inspection.
   - Start-up order changed slightly: the shell (session, prefs, files, control server) is now built inside eframe's creator closure, i.e. after the window/GPU exist rather than before `run_native`. This removes the old `pending` Cell; on the Windows graphics retry the shell is built only on the attempt that reaches creation (`created` flag), so the retry semantics are preserved. 
   - `can_close()` is new (the old app had no equivalent); `Shell::logic` still cancels close only while printing, exactly as before.
   - `Launch` carries `prefs`/`config_dir` rather than the shell recomputing them, so the surface is exactly the brief's plus public `Launch` fields.
   - Docs `docs/egui-037-migration-plan.md` mention old paths (left untouched).

7. **git status --porcelain:**
```
 M Cargo.lock
 M Cargo.toml
 M apps/iridium-sheets/Cargo.toml
 M apps/iridium-sheets/src/main.rs
RM apps/iridium-sheets/src/apple_events.rs -> crates/sheets-shell/src/apple_events.rs
R  apps/iridium-sheets/src/connector.rs -> crates/sheets-shell/src/connector.rs
R  apps/iridium-sheets/src/control_server.rs -> crates/sheets-shell/src/control_server.rs
R  apps/iridium-sheets/src/gsheets.rs -> crates/sheets-shell/src/gsheets.rs
R  apps/iridium-sheets/src/native_menu.rs -> crates/sheets-shell/src/native_menu.rs
R  apps/iridium-sheets/src/print.rs -> crates/sheets-shell/src/print.rs
R  apps/iridium-sheets/tests/mac_menu_appkit.rs -> crates/sheets-shell/tests/mac_menu_appkit.rs
 M xtask/src/layers.rs
?? crates/sheets-shell/Cargo.toml
?? crates/sheets-shell/src/lib.rs
```
