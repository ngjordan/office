# PHASE 1 — Decks

**VERDICT: SHELL LIBRARY DONE**

## Gate
`cd /opt/hermes-agent/workspace/dc-deckcraft && CARGO_BUILD_JOBS=4 CRAFT_FONTS_DIR=/tmp/craft-fonts CRAFT_I18N_DIR=/tmp/craft-i18n PKG_CONFIG_PATH=.../alsa-stub/pkgconfig LIBRARY_PATH=.../alsa-stub/lib cargo xtask ci` → exit 0 (fmt, pdf-deps, clippy, test, doctest, assets, layers, wasm all passed).

## New crate
`crates/decks-shell/`, package `iridium-decks-shell` (lib, L7 in xtask layers table as `("shell", Class::Layer(7))` — table uses names minus `iridium-decks-`). Added to `[workspace.dependencies]` (members glob `crates/*` already covers it). wasm gate excludes it automatically (only L<=5 + ui-egui are selected; verified in the wasm output). No new external deps (rfd, arboard, image, cpal, eframe, egui, serde_json, log, + internal engine/ui-egui/media; fmv-macos-events on macOS).

API (exact):
```rust
#[derive(Default)]
pub struct Launch { pub control_port: Option<u16>, pub files: Vec<String>, pub sample: bool, pub show: bool,
                    #[cfg(target_os="macos")] pub apple_events: fmv_macos_events::Inbox }
pub struct Shell { /* private */ }
impl Shell {
  pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self;
  pub fn logic(&mut self, ctx: &egui::Context);
  pub fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput);
  pub fn ui(&mut self, ui: &mut egui::Ui);
  pub fn open_path(&mut self, path: &str);   // errors logged + shown in status line (same as old Apple-event path)
  pub fn save_state(&mut self);
  pub fn can_close(&self) -> bool;
}
```

## Moved out of main.rs (into the crate)
- `SlideApp` construction: `install_ime_pass_hooks`, `services()`, `load_prefs`, `config_dir` (copy), AutoRecover (`recovery_dir`, `file.recovery.open`), `enable_update_checks`, titlebar flag, control server start, `--sample`, `open_from_system`, `--show`.
- Files moved verbatim (plain mv): `audio.rs`, `pasteboard.rs`, `control_server.rs`, `control_server/tests.rs` (their tests now run in the shell crate).
- `save_prefs` → `Shell::save_state` (+ macOS `cleanup_print_previews`, same order: save, cleanup, then exit).
- macOS `apple_events::poll` → inlined in `Shell::logic` (needs `SlideApp`); the `Inbox` arrives via `Launch.apple_events`.
- Deps removed from the app Cargo.toml: engine, media, serde_json, rfd, arboard, image, cpal.

## Stayed in main.rs
- `logging` / `log::set_logger` and log-dir attach (`config_dir` kept as a second copy for the log path — 12 lines duplicated rather than widen the API).
- `craft_update::self_update::finish`, arg parsing (`startup`, USAGE, `--help/--version`), `app_icon`, NativeOptions, `display` (X11 choice), `graphics::configure`, `apple_events::AppleEvents::install` (must precede the event loop; process-wide).
- macOS `process::exit(0)`, now in `fn end_process()` called from `on_exit` after `save_state`. main.rs: 458 → ~310 lines (rest is its tests).
- Test `on_exit_ends_the_process_on_macos` can no longer build a `Shell` (needs an eframe CreationContext), so its child now calls `end_process()` directly; same assertion.

## can_close
Decks has an unsaved-changes prompt: `SlideApp::logic` (app.rs:241) cancels a close request and opens the "quit" dialog when `session.documents().iter().any(|d| d.is_dirty())`. `can_close` = `!any(is_dirty)` — the same predicate; no new tracking. It does NOT cover "job in flight" (Decks has no such flag).

## Obstacles
- First gate run: `fmt` failed (`if launch.sample && let Err(..)` needed wrapping per rustfmt.toml); fixed with `cargo fmt --all`. Otherwise no errors.
- Pre-existing, untouched: warning `field opening_generation is never read` in ui-egui; unused-patch warnings.

## Could not do
- Could not compile/run the macOS-only code paths (`Launch.apple_events`, inlined poll, `end_process` exit) on this Linux box; written to mirror the old code exactly but unverified. No GUI launch smoke-test performed (headless); behaviour preserved by code move, tests pass.
- Did not edit stale doc references to moved files (docs/egui-037-migration-plan.md, README mention logging.rs which stays).

## git status --porcelain
```
 M Cargo.lock
 M Cargo.toml
 M apps/iridium-decks/Cargo.toml
 M apps/iridium-decks/src/apple_events.rs
 D apps/iridium-decks/src/audio.rs
 D apps/iridium-decks/src/control_server.rs
 D apps/iridium-decks/src/control_server/tests.rs
 M apps/iridium-decks/src/main.rs
 D apps/iridium-decks/src/pasteboard.rs
 M xtask/src/layers.rs
?? crates/decks-shell/
```
