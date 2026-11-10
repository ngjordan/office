# PHASE2 — CAD

1. **VERDICT: HOST API DONE**
2. Gate: `cd /opt/hermes-agent/workspace/cc-cadcraft && export CARGO_BUILD_JOBS=4 CRAFT_FONTS_DIR=/tmp/craft-fonts CRAFT_I18N_DIR=/tmp/craft-i18n umask 022 && cargo xtask ci` → exit 0, "all 7 steps passed (fmt, clippy, test, i18n, assets, layers, wasm)".
3. Signatures (`crates/cad-shell/src/lib.rs`):
   - `pub fn new_for_host(ctx: &egui::Context, storage: Option<&dyn eframe::Storage>, launch: Launch) -> Self`
   - `pub fn fonts(&self) -> egui::FontDefinitions` / `pub fn style(&self) -> egui::Style` / `pub fn zoom(&self) -> f32`
   - `pub fn install_native_menu(&self)` (macOS: sets an `AtomicBool`; the next `logic` drops and rebuilds the menu, because `&self` can't mutate. Other OSes: no-op)
   - `pub fn new(cc, launch) -> Self` (wrapper, unchanged signature)
   - Supporting changes in `iridium-cad-ui-egui`: new `pub fn CadApp::install_look(&mut self, ctx)`, new `pub installed_fonts: Option<FontDefinitions>` field, new `pub fn theme::font_definitions(ctx, language) -> FontDefinitions`. `theme::install_fonts` keeps its signature. Nothing public renamed or removed. Style comes from `ctx.global_style()` (egui 0.37 has no `ctx.style()`).
4. Look call-site audit (non-test). **CAD installs its look lazily, NOT at construction, and checks again every frame.** `CadApp::new` has no ctx. All sites run from `ui_frame_begin` (called from `CadApp::ui`, lib.rs:794 → now via `install_look`), every UI frame but change-gated. None run from `logic`.
   - `set_fonts`: `crates/ui-egui/src/theme.rs:401`, inside `install_fonts`. `install_fonts` is now a wrapper that calls `font_definitions` and then `set_fonts`. It is called in `ui-egui/src/lib.rs` `install_look` when the language changed. The language can change at runtime (fonts_language gate), so the captured fonts go stale on a language switch; the user can't pick a font directly.
   - `set_visuals_of` ×2: `theme.rs:792`, `:793` in `theme::apply`, plus `set_theme` `:796` and `all_styles_mut` `:803` (style). `animation.rs:58` (`all_styles_mut`, scroll animation). Called from `install_look` when the theme or follows-system changed, or the animation setting changed. Both are re-evaluated every frame, so theme or animation changes at runtime go stale in the capture.
   - `set_zoom_factor`: `ui-egui/src/lib.rs:858` (now in `install_look`), applied when `options.interface_scale` changes. egui's keyboard zoom is separate.
   - Because the look is lazy, `new_for_host` calls `app.install_look(ctx)` at construction so the capture is real. Without this it would capture egui defaults. The standalone app then installs the same things one frame earlier. `ui_frame_begin` calls `install_look` again each frame, where it is a no-op when nothing changed.
5. Stays in `Shell::new`: the wgpu render-state wiring (`cc.wgpu_render_state` → `CadApp::set_wgpu`). `cc.wgpu_render_state` is only on `CreationContext`. A host sets wgpu itself. CAD has no other cc-only extra. The 3 `cc.storage` reads moved to `new_for_host` and use the `storage` parameter. `None` skips all three reads and falls back to defaults, with no panic path (`and_then`).
   - Order change: `set_wgpu` now runs after control-server start, file opening and `install_look`, where it used to run before them. It is only a state setter, so I don't expect an effect.
6. Obstacles:
   - `error[E0599]: no method named 'style' found for reference '&egui::Context'` → used `global_style()`.
   - `ctx.fonts(..)` panics ("No fonts available until first call to Context::run()") before the first pass, and it wouldn't show pending `set_fonts` anyway. I record the `FontDefinitions` the app built instead.
   - Changing `install_fonts`' return type broke 2 test files (`error[E0308]: mismatched types ... expected (), found FontDefinitions` in `tests/it/properties_units.rs:21` and `shared_controls.rs:121`). Reverted to the original signature and added `font_definitions`.
   - A first background launch via `nohup` outside the terminal had no cargo on PATH (exit 127). I re-ran it properly.
7. Not done / unverified: the macOS paths (`install_native_menu` rebuild, `native_menu.rs`) are `cfg(macos)` and can't compile or run on this Linux host. I wrote them without compiling. Launching or printing was not exercised manually. Only the gate and tests ran. Host-side `Shell::new_for_host` with `storage=None` was not run as a test.
8. `git status --porcelain`:
```
 M crates/cad-shell/src/lib.rs
 M crates/ui-egui/src/lib.rs
 M crates/ui-egui/src/theme.rs
```
