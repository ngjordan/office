# PHASE 1 — Cad (cc-cadcraft)

1. **VERDICT: SHELL LIBRARY DONE** (with two deviations from the brief, see §3/§4)

2. **Gate:** `cd /opt/hermes-agent/workspace/cc-cadcraft && export CARGO_BUILD_JOBS=4 CRAFT_FONTS_DIR=/tmp/craft-fonts CRAFT_I18N_DIR=/tmp/craft-i18n; umask 022; cargo xtask ci` → **exit 0** ("all 7 steps passed (fmt, clippy, test, i18n, assets, layers, wasm)"). Layers shows `iridium-cad-shell L6 engine, io, mcp, ui-egui`; wasm set is 11 crates, shell not in it (the wasm filter takes Layer<=4 + ui-egui, so L6 is excluded automatically).

3. **Crate:** `crates/cad-shell/`, package `iridium-cad-shell` (v0.17.0), lib `iridium_cad_shell`. Registered: root `Cargo.toml` `[workspace.dependencies]` (members glob `crates/*` covers it), `xtask/src/layers.rs` TABLE `("shell", Class::Layer(6))` (names there are stripped of `iridium-cad-`). No new external deps.
```rust
pub struct Launch { pub control: Option<u16>, pub sample: bool, pub files: Vec<PathBuf>,
  pub config_dir: Option<PathBuf>, pub system_cursor: bool, pub locale: Option<String>,
  pub no_native_menu: bool, pub frame_cap: bool,
  pub font_alt: Option<String>, pub font_fallback: Option<String> }   // Debug, Clone, Default
pub struct Shell { /* private */ }
impl Shell {
  pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self;
  pub fn logic(&mut self, ctx: &egui::Context);
  pub fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput);
  pub fn ui(&mut self, ui: &mut egui::Ui);
  pub fn open_path(&mut self, path: &str);          // -> CadApp::open_path(Path::new(path))
  pub fn save_state(&mut self);
  pub fn can_close(&self) -> bool;                  // !cad.opening.busy() && no doc is_dirty()
  pub fn save_prefs(&self, storage: &mut dyn eframe::Storage);   // EXTRA — see below
}
```
**Deviation 1 (extra method):** in this repo the prefs (profile, prefs, autosave prefs) are written by `eframe::App::save(storage)`, not `on_exit`. `save_state()` has no storage handle, so `save_prefs(storage)` is an additional public method that main's `save` calls. Phase 2 must forward eframe's `save` to it (or drop prefs persistence in the suite).
`save_state()` = what `on_exit` did: workspace save, `cancel_screenshots`, `shutdown_autosave`. `can_close` reuses `Opening::busy` + `Document::is_dirty` (same test as `request_quit`); no new tracking.

4. **Moved to the shell:** `control_server`, `desktop_theme`, `native_menu` (macOS), `spacemouse`(+`motion`), `system_animations`, `workspace` (git mv, with their tests); the `App` struct and its `logic`/`raw_input_hook`/`ui`/frame code; `follow_desktop`; `services()`, `filters_for_save`, `install_io()` (+tests); per-app init that used to precede `run_native` (`install_io`, `settings_dir::set`, fault report hook, font alt/fallback) now runs in `Shell::new`; the whole CadApp construction closure body (workspace/profile/prefs restore, autosave, LISP dir, wgpu, control, sample, file opens, devices). Deps moved with them (zbus, hidapi, windows, muda, objc2-app-kit, craft-window, rfd, sys-locale). One test (`profile_override_isolated_or_explicitly_disabled`) used `crate::env::Env`; reduced to `no_settings_folder_means_no_workspace_file` (the env parsing is still covered by `env.rs` tests in the app).
   **Stayed in main.rs:** `dwg_worker` early exit, `logging::install`/`attach_dir` (log::set_logger), `craft_update::self_update::finish` updater call, `launch_args`/`env` parsing, `graphics.rs` + `graphics::configure`, wgpu surface/limits config, `window_options`/`saved_window`/app icon/`APP_ID`, X11 (Linux) and macOS `with_close_requested_on_terminate` event-loop builders, `eframe::run_native`, a thin `App{shell}` eframe adapter. `STENCIL_BITS=8` is duplicated in the shell (private const) because `set_wgpu` needs it.
   **Deviation 2:** the brief says on_exit holds a macOS `std::process::exit(0)`. **It does not in this repo** — no such call exists (only `exit` for dwg worker/updater failure in main). Nothing was added; behaviour unchanged.

5. **Obstacles:**
   - `cannot find value STENCIL_BITS` in shell → private const copy.
   - `expected &Path, found &String` → `Launch.files` is `Vec<PathBuf>` (launch args already are PathBuf; avoids lossy UTF-8).
   - `cannot find crate iridium_cad_mcp` in `launch_args.rs` (uses `args` helpers) → kept `iridium-cad-mcp` dep in the app.
   - My merge duplicated the desktop-theme/animations starter lines (spawns two watcher threads; warned unused var) → removed the duplicate. Also unused `OsString` import in workspace tests.
   - No gate greps `main.rs` (xtask checked). Only `.github/workflows/ast-grep.yml` path-filters `apps/iridium-cad/src/main.rs`/`launch_args.rs` (both still exist; not edited per AGENTS.md CI rule).

6. **Not done / unverified:** macOS and Windows code paths (native menu, `graphics::configure`, spacemouse on Windows) are moved but compiled only for Linux here. I did not launch the GUI (no display check); verification is compile + the full test suite + clippy. Tests in `iridium-cad` (bin) now contain only launch-args/env/logging/graphics/window tests; app-behaviour tests moved to `iridium-cad-shell`.

7. **`git status --porcelain`:**
```
 M Cargo.lock
 M Cargo.toml
 M apps/iridium-cad/Cargo.toml
 M apps/iridium-cad/src/main.rs
R  apps/iridium-cad/src/control_server.rs -> crates/cad-shell/src/control_server.rs
R  apps/iridium-cad/src/desktop_theme.rs -> crates/cad-shell/src/desktop_theme.rs
R  apps/iridium-cad/src/native_menu.rs -> crates/cad-shell/src/native_menu.rs
R  apps/iridium-cad/src/spacemouse.rs -> crates/cad-shell/src/spacemouse.rs
R  apps/iridium-cad/src/spacemouse/motion.rs -> crates/cad-shell/src/spacemouse/motion.rs
R  apps/iridium-cad/src/system_animations.rs -> crates/cad-shell/src/system_animations.rs
RM apps/iridium-cad/src/workspace.rs -> crates/cad-shell/src/workspace.rs
 M xtask/src/layers.rs
?? crates/cad-shell/Cargo.toml
?? crates/cad-shell/src/lib.rs
```
(renames are staged by `git mv`; nothing committed.)
