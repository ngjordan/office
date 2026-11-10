# PHASE2 — PDF (pdfcraft, `iridium-pdf-shell`)

1. **VERDICT: HOST API DONE**

2. **Gate** (exit 0, "CI: all 10 steps passed"):
`cd /opt/hermes-agent/workspace/pdfcraft && export CARGO_BUILD_JOBS=4 CRAFT_FONTS_DIR=/tmp/craft-fonts CRAFT_I18N_DIR=/tmp/craft-i18n PKG_CONFIG_PATH=/opt/hermes-agent/workspace/alsa-stub/pkgconfig LIBRARY_PATH=/opt/hermes-agent/workspace/alsa-stub/lib; umask 022; cargo xtask ci` → EXIT=0. Log: /tmp/pdf-ci.log.
Not run: the standalone app on a display. No-behaviour-change is argued from the code path below, not exercised.

3. **Signatures** (`crates/pdf-shell/src/lib.rs`, only file changed):
```rust
pub fn new_for_host(ctx: &egui::Context, storage: Option<&dyn eframe::Storage>, launch: Launch) -> Self;
pub fn fonts(&self) -> egui::FontDefinitions;
pub fn style(&self) -> egui::Style;
pub fn zoom(&self) -> f32;
pub fn install_native_menu(&mut self);   // &mut self: PdfApp keeps the built menu in self.native_menu
```
`Shell::new(cc, launch)` is unchanged in signature. `Shell` gained private fields `ctx`, `fonts`, `style`, `zoom`. `Launch` unchanged; no new deps.

   `install_native_menu` re-calls `PdfApp::install_native_menu(&mut self, ctx)` (menus.rs:608) with the stored `ctx` clone. It is a no-op off macOS. That path is cfg-gated and was NOT compiled here (Linux host).

4. **Look call-site audit** (non-test; test sites checked against each file's `#[cfg(test)]` line):
   - `set_fonts` ← `theme.rs:216` (`install_fonts_for`). Callers: `app.rs:541` (first frame only, under `!self.styled`) and `app.rs:549` (when the UI language's CJK preference changes). Both run in `logic_unguarded`, so **lazy: on the first frame, then on language change. Never at construction.**
   - `set_fonts` ← `tools/edit_text_ui/layout.rs:105` (`install_source_fonts`): **runtime, per document or editor open.** Callers: `panels/edit.rs:459`, `tools/edit_text_ui.rs:672`, `workflows/files.rs:584`. It reads `ctx.fonts()` and adds `pdf-source-*` faces. The captured font set never contains these.
   - `app.rs:737` is a comment only.
   - `widgets.rs:729/854/875/913/947` (tests start at 713), `home.rs:291/326` (279), `dialogs/appearance.rs:204/248` (186), `tools/field_table_ui.rs:490` (453), `tools/content_ui.rs:1497` (1400) and `tools/redact_ui.rs:707/737` (688) are all **tests**. The brief listed `widgets.rs:729` as non-test; it is a test.
   - `set_visuals` / `global_style_mut` / `set_theme` / `set_style_of` ← `theme::apply` (`theme.rs:430,473,474,492,493`). Callers: `app.rs:543` (first frame), `app/frame.rs:32` (`sync_theme`, called each frame from `app.rs:552` but applies only when the resolved theme kind changes), and `app/prefs.rs:43` (`set_theme_preference`, on user or control-channel change). **First frame, then on theme change.**
   - `set_zoom_factor` ← `platform/ui_scale.rs:69` (`ui_scale::sync`), called from `app.rs:555` each frame in `logic`. It sets the zoom only when the factor differs from the last applied one. **Per frame, but effectively on change.** It also sets `zoom_with_keyboard = false` on the first frame.
   - **PDF's look is lazy and changes at runtime** (language, theme, UI scale, text scale), as with Docs.

5. **What stayed in `Shell::new`, and why.** Everything that needs the real `CreationContext`:
   - Windows taskbar icon: `cc.winit_window()` → `set_taskbar_icon`. It now runs before construction rather than midway; nothing depends on the order.
   - wgpu report: log line plus `notify_software_renderer(&mut app, device_type)`. It stays in construction and takes `window.wgpu: Option<&egui_wgpu::RenderState>`, so a software-renderer notice is raised at the same point. Skipping it would change behaviour, because it raises a user-visible warning. The host path passes `None` and logs "renderer: OpenGL (glow)". I did not add a `Launch` field.
   - Wayland check: `native_wayland(cc)` is evaluated in `new` and passed as `window.wayland`. The host path passes `false`, so `os_events` receives `false`.
   - **Deviation from the brief's sketch:** `new` does NOT call `new_for_host`. Both call a private `new_with(ctx, storage, launch, Window { wayland, wgpu })`. A literal `new` → `new_for_host` wrapper could not carry the wgpu info to the spot where the notice is raised. It would also have changed the notice order and the early-return path. Standalone behaviour is identical.
   - Everything else moved into the private `build` unchanged. `cc.storage` became the `storage` parameter at both sites (state restore, `WindowState::started_with`). `cc.egui_ctx` became `ctx`. With `storage = None`, `started_with(None)` and the restore skip already handled it with no panic; no new code path.
   - The `startup_error` early return is preserved (`build` returns the app early; the look is still captured).

6. **Obstacles**
   - First gate run, clippy failed: `error: field 'ctx' is never read --> crates/pdf-shell/src/lib.rs:56:5` (`-D dead-code`; the field is only read on macOS). Fix: `#[cfg_attr(not(target_os = "macos"), allow(dead_code))]` on the field. Re-run was green.
   - A combined edit-and-nohup command was rejected by the tool. The rejection discarded the edit, which cost one wasted gate run before I noticed.

7. **Not done / limits**
   - Capturing at construction: `fonts` is the set PDF builds, `theme::installed_font_definitions(CjkPreference::for_language(app.language))`. It is not read back from `ctx`, which would panic. It is the base set plus whatever system fallback faces exist on the machine (it reads system fonts at construction, native only). It excludes `pdf-source-*` document fonts and any later language switch.
   - `style` and `zoom` are what the context held at construction (egui defaults, or the host's previous module's look), because PDF installs its look in its first `logic`. I did not add an eager install or warm-up, since that would change standalone behaviour. The host must run PDF's `logic` after a switch rather than rely on `style()` or `zoom()`.
   - macOS `install_native_menu` is untested here.

8. **`git status --porcelain`**
```
M crates/pdf-shell/src/lib.rs
```

## FIX: the captured look is PDF's own

**Verdict:** done. `Shell::style()` / `zoom()` now return what PDF installs, not egui's defaults.
**Gate:** `cd /opt/hermes-agent/workspace/pdfcraft && CARGO_BUILD_JOBS=4 ... cargo xtask ci` → "CI: all 10 steps passed", exit 0.

**Signatures (real):**
- `PdfApp::install_look(&mut self, ctx: &egui::Context) -> f32` (new, `crates/ui-egui/src/app.rs`; returns the zoom applied)
- unchanged: `Shell::new(cc, launch)`, `new_for_host(ctx, storage, launch)`, `fonts()`, `style()`, `zoom()`, `install_native_menu(&mut self)`, `Launch`.

**What changed:**
- `app.rs`: the first-frame font/theme block, `sync_theme`, and `ui_scale::sync` moved verbatim out of `logic_unguarded` into `install_look`; `logic_unguarded` calls it each frame. Same functions as before (`theme::install_fonts_for`, `theme::apply`, `sync_theme`, `ui_scale::sync`, `install_image_loaders`).
- `pdf-shell/src/lib.rs` `new_with` (shared by `new` and `new_for_host`): calls `app.install_look(ctx)` before capturing; zoom is the returned value. fonts still `installed_font_definitions(..)` (no `ctx.fonts()`); style still `ctx.global_style().as_ref().clone()`; `ctx` keeps its `cfg_attr(allow(dead_code))`; `startup_error` early return, Windows icon and wgpu report untouched (standalone path only). Doc comments updated.
- Why `zoom` is returned: egui's `set_zoom_factor` only shows in `zoom_factor()` after the next pass, so reading it back gave 1.0 (found by a failing test at 1.5).

**Standalone behaviour:** identical look/fonts/visuals/zoom/renderer warning. Only difference: the look is installed one frame earlier (at construction); `fonts_ready` is consequently true from frame 1 (fonts are applied in the first pass).

**Proof:** test `tests::the_captured_look_is_the_apps_own_not_eguis_default` (pdf-shell): builds `Shell::new_for_host` on a fresh context, asserts captured `visuals.panel_fill` != egui default, == the context's, and `zoom()` == `ctx.zoom_factor()` after one pass. Output: `test ... ok`, 1 passed. With the install removed (mutant) it fails: `assertion left != right failed: PDF's visuals, not egui's`.

**Obstacles:** `Dropped TexturesDelta with 1 unapplied deltas` (test panic, fixed by `textures_delta.clear()`); zoom read-back 1.0 vs 1.5 (above). `layers.rs` passes (in gate).
**Not done:** zoom is not distinguishable from egui's default in the test (default PDF zoom is 1.0); style is the discriminator.

**git status --porcelain:**
    (empty: the tree is clean because another actor committed my changes as 9848a52a1 "Capture the app's own look, not egui's defaults" while the gate ran; I did not commit.)
