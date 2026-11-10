# PHASE2 — Sheets (gc-gridcraft, crates/sheets-shell)

1. **VERDICT: HOST API DONE**
2. Gate: `cd /opt/hermes-agent/workspace/gc-gridcraft && export CARGO_BUILD_JOBS=4 CRAFT_FONTS_DIR=/tmp/craft-fonts CRAFT_I18N_DIR=/tmp/craft-i18n && umask 022 && cargo xtask ci` → exit 0, "all 10 steps passed". (Linux only: the macOS-cfg code was NOT compiled here.)
3. Signatures (all in `impl Shell`, `crates/sheets-shell/src/lib.rs`):
   - `pub fn new_for_host(ctx: &egui::Context, _storage: Option<&dyn eframe::Storage>, launch: Launch) -> Self`
   - `pub fn fonts(&self) -> egui::FontDefinitions`
   - `pub fn style(&self) -> egui::Style`
   - `pub fn zoom(&self) -> f32`
   - `pub fn install_native_menu(&self)` (macOS: builds a new NativeMenu from a stored `ctx` clone and replaces `menu`; honours IRIDIUM_NO_NATIVE_MENU; no-op elsewhere)
   - `Shell::new(cc, launch)` = `Self::new_for_host(&cc.egui_ctx, cc.storage, launch)`.
   - Shell gained fields `fonts`, `style`, `zoom` (captured after `setup_context_for_language`, via `ctx.fonts(|f| f.definitions().clone())`, `ctx.global_style()`, `ctx.zoom_factor()`), and macOS-only `ctx`; macOS `menu` became `RefCell<Option<NativeMenu>>` (accessed via `get_mut()` in `logic`). No Launch change, no new deps.
4. Call-site audit (non-test):
   - `ui-egui/src/app.rs:111` set_fonts — in `setup_context_for_language`: construction AND per frame (called from fonts.rs:84 via `start_font_setup`, run from `logic` when `fonts_set` is reset, fonts.rs:61/64).
   - `ui-egui/src/app/fonts.rs:81` set_fonts(pinned) — per frame path (`start_font_setup`, only when `prepared_fonts`).
   - `ui-egui/src/app/fonts.rs:107` set_fonts(with workbook fonts) — per frame (`rebind_workbook_fonts`), runtime: fires when open workbooks use new fonts → captured copy goes stale.
   - `ui-egui/src/ribbon/title_bar.rs:233`, `formula_bar.rs:1026`, `widgets.rs:726`, `app.rs:373`: tests only.
   - set_visuals: `theme.rs:334` (`set_visuals_of`) and `set_theme` at `theme.rs:337`, inside `theme::apply` — construction and again from `start_font_setup` (per-frame path); `formula_bar.rs:1030` is test.
   - set_theme: `app/prefs.rs:24` — per frame (`sync_theme_preference`, from `logic` app.rs:130) when preference differs.
   - set_zoom_factor: `app/prefs.rs:46` — per frame (`sync_input_options`, app.rs:131), only when zoom differs from `ui.zoom_factor()`.
   - Note: `logic` also does `all_styles_mut` in `theme::apply`. Sheets changes fonts at runtime (workbook fonts rebind, language change) → host re-issuing the captured set is stale-prone.
5. Stayed in `Shell::new`: nothing beyond the wrapper — Sheets uses only `cc.egui_ctx` and (ignored) `cc.storage`; no winit_window/platform extras. Everything else, including the control server and macOS inbox `set_wake`, uses `ctx` and lives in `new_for_host`. Storage `None` is safe (Sheets never reads storage; prefs come from `launch.config_dir`).
6. Obstacles: `error[E0599]: no method named \`style\` found for reference \`&egui::Context\`` (egui 0.37 fork) → used `ctx.global_style()`. First gate run failed at clippy on that; second passed.
7. Not done: macOS code paths (`install_native_menu`, `RefCell` menu) not compiled/tested on this Linux host; no runtime launch/print test of the standalone app (gate tests only).
8. `git status --porcelain`: see below.
 M crates/sheets-shell/src/lib.rs
