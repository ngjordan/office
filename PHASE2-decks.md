# Phase 2 — Decks (`crates/decks-shell`)

1. **VERDICT: HOST API DONE**
2. Gate: `cd /opt/hermes-agent/workspace/dc-deckcraft && export CARGO_BUILD_JOBS=4 CRAFT_FONTS_DIR=/tmp/craft-fonts CRAFT_I18N_DIR=/tmp/craft-i18n umask 022 PKG_CONFIG_PATH=/opt/hermes-agent/workspace/alsa-stub/pkgconfig LIBRARY_PATH=/opt/hermes-agent/workspace/alsa-stub/lib && cargo xtask ci` -> **exit 0**, "all 8 steps passed (fmt, pdf-deps, clippy, test, doctest, assets, layers, wasm)". Log: /tmp/decks-p2-ci.log. Layer table needed no change.
3. Signatures (impl Shell, crates/decks-shell/src/lib.rs):
   - `pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self` (now `Self::new_for_host(&cc.egui_ctx, cc.storage, launch)`)
   - `pub fn new_for_host(ctx: &egui::Context, _storage: Option<&dyn eframe::Storage>, launch: Launch) -> Self`
   - `pub fn fonts(&self) -> egui::FontDefinitions`, `pub fn style(&self) -> egui::Style`, `pub fn zoom(&self) -> f32`, `pub fn install_native_menu(&self)` (no-op on all platforms: Decks builds no native menu; menu bar is egui).
   - `Shell` gained private fields `fonts`, `style`, `zoom`. `Launch` unchanged.
4. Call-site audit (non-test). **KEY FINDING: Decks does NOT install fonts/style/zoom at construction.** `SlideApp` does it lazily on the first frame and zoom every frame:
   - `crates/ui-egui/src/app.rs:286` `logic` -> `ui_setup` (app/frame.rs) each frame.
   - `set_zoom_factor`: `crates/ui-egui/src/app/frame.rs:9` — **PER FRAME** (only when `ctx.zoom_factor()` differs from `ui.ui_scale.clamp(0.5,3.0)`, i.e. effectively on first frame + when user changes scale). Host switching modules should re-issue zoom (`Shell::zoom()`); otherwise another module's zoom is reset by Decks next frame anyway.
   - `set_fonts`: `theme.rs:182` (`install_fonts_with`), called from `app/frame.rs:12` (once, first frame, `styled` flag), `uifonts.rs:130` (`UiFonts::apply`, called each frame from `app/ime.rs:30` — **PER FRAME, only when a needed fallback face set changes**, so fonts change at runtime), `presenter.rs:121` (presenter window, once). Other `set_fonts` hits (`theme.rs:279,349,392,418`) are inside `#[cfg(test)]`. `app/frame.rs:5` is a comment.
   - `set_visuals`: `theme.rs:161` (in `theme::apply`, called from `app/frame.rs:~21` on `restyle`, first frame and on brightness change; and `presenter.rs:122`). `ribbon.rs:376` is test code. `global_style_mut` (style): `theme.rs:162`, same callers.
   - Mitigation: because first-frame install is lazy, `new_for_host` now performs the same three installs eagerly (`ctx.set_zoom_factor`, `ctx.set_fonts(theme::font_definitions(&app.ui_fonts.faces))`, `theme::apply(ctx, Tokens::for_brightness(app.ui.brightness))`) and captures them. First frame repeats them (idempotent; `set_fonts` skips equal defs). At construction `ui_fonts.faces` is empty, so captured fonts = base set; runtime fallback faces added later (Hangul, CJK, ...) are **not** in the capture (stale-capture case the brief anticipates; the app re-adds them itself on its next frame).
   - Fonts captured from `theme::font_definitions(..)` rather than `ctx.fonts()`: `Context::fonts()` panics ("No fonts available until first call to Context::run()") before the first pass, and `set_fonts` applies at next pass anyway.
5. Stayed in `Shell::new`: nothing platform-specific. Decks has no `cc.winit_window()` use; `new` only forwards `cc.egui_ctx`/`cc.storage`. Storage is ignored (settings are files via `config_dir`), so `None` is safe.
6. Obstacles: first attempt using `nohup … &` was rejected by the tool; reran with tracked background. `egui` source for fonts()/global_style() checked in the cargo git checkout. No compile errors.
7. Not done: no behavioural/screenshot test of standalone launch (no display). One deliberate deviation from "construct exactly as today": standalone `new` now also installs fonts/theme/zoom before the first frame (the first frame did the same). Pre-existing warning `opening_generation never read` in ui-egui (untouched; gate passes).
8. `git status --porcelain`:
   ` M crates/decks-shell/src/lib.rs`
