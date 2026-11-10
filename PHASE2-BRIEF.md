# Phase 2 brief — give each app's `Shell` a host-facing API

## Why this exists

Phase 1 moved each app's shell into a library crate. Phase 2 builds the suite (one window, module
switcher) on top of it. The suite must be able to:

1. construct a module **later** than app start-up (lazy start), and
2. restore a module's look (fonts / style / zoom) when it becomes active again.

Today `Shell::new` takes `&eframe::CreationContext<'_>`. That type cannot be stored: it holds
`storage: Option<&'s dyn Storage>` (a borrow) and its window fields are `pub(crate)`. So a host
cannot construct a module after the eframe creator closure has run. **That is the one thing this
phase must unblock.**

## The change

Add to each of the five shell crates a **host-facing constructor** plus a few look accessors.
The existing `Shell::new(cc, launch)` must keep working and keep behaving exactly as it does today
— it becomes a thin wrapper.

```rust
// in crates/<app>-shell/src/lib.rs

impl Shell {
    /// Host-facing constructor: no `CreationContext`, so a host can build this module later.
    ///
    /// `ctx` is the host's shared egui context. `storage` is the host's persistent store, if any
    /// (`None` when the host has none).
    pub fn new_for_host(
        ctx: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
        launch: Launch,
    ) -> Self;

    /// The font set this module installed, captured at construction.
    pub fn fonts(&self) -> egui::FontDefinitions;

    /// The style this module left behind, captured at construction.
    pub fn style(&self) -> egui::Style;

    /// The zoom factor this module left behind, captured at construction.
    pub fn zoom(&self) -> f32;

    /// (macOS) Re-install this module's native menu. No-op on other platforms.
    pub fn install_native_menu(&self);
}
```

`Shell::new(cc, launch)` becomes:

```rust
pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self {
    // platform extras that need the real window (PDF's Windows taskbar icon), then:
    Self::new_for_host(&cc.egui_ctx, cc.storage, launch)
}
```

Keep every existing platform-specific extra that needs `cc` in `new` (not in `new_for_host`).
PDF's Windows taskbar icon is the known example — it uses `cc.winit_window()`, which a host
constructor does not have. Say so in your report.

### Capturing the look

`new_for_host` must, in this order:

1. construct the app exactly as `new` does today,
2. let the app install its fonts / style / zoom into `ctx`,
3. **capture** `ctx.fonts()`, `ctx.style()` and `ctx.zoom()` into the `Shell`.

Capture *after* the app has installed them, otherwise you capture the host's previous module's look.

If a module changes its fonts later at runtime (a user picking a font), the captured copy goes
stale — the host re-issues the captured set on switch, which is what the plan requires for v1.
Note in your report if your app changes fonts at runtime.

## Rules

1. **No behaviour change.** The standalone app must launch, open files, print, show its native menu
   and quit exactly as before. `Shell::new` must remain the path the standalone app takes.
2. **Do not rename or remove anything public.** The suite will be written against the surface above.
3. **Do not add a trait.** Each crate exposes one concrete `Shell`, as in Phase 1.
4. **No new external dependencies.**
5. `Launch` keeps its current shape. If `new_for_host` genuinely needs a field `Launch` does not
   have, add it to `Launch` (public struct) — do not add parameters to `new_for_host` beyond the
   three above.
6. `storage` may be `None`; every app must still work when it is (the suite may host without a
   persistent store). Check what your app does with prefs when storage is `None` and make sure it
   does not panic.
7. Follow the repo's `AGENTS.md`: layering, no panics, no `unsafe`, asset policy, quality gates.
8. No commits, no branches, no pushes. Leave the changes in the working tree.

## The gate

```
cd <repo> && export CARGO_BUILD_JOBS=4 CRAFT_FONTS_DIR=/tmp/craft-fonts CRAFT_I18N_DIR=/tmp/craft-i18n umask 022 && cargo xtask ci
```

Add the ALSA stub for Decks and PDF:
`export PKG_CONFIG_PATH=/opt/hermes-agent/workspace/alsa-stub/pkgconfig LIBRARY_PATH=/opt/hermes-agent/workspace/alsa-stub/lib`

It MUST end green. The first build after your change is slow — run it in the background, write to a
log under `/tmp/`, and poll it. 20-45 minutes of compiling is normal. A timeout is not a failure.

## Facts already measured — do not rediscover these

- **Construction is cheap.** Building all five app types costs ~14 ms total (sheets 373 µs, docs
  13 ms, cad 111 µs, decks 104 µs, pdf 175 µs). Lazy start is about correctness and side effects
  (control servers, native menus, fonts), not about CPU time.
- **`set_fonts` call sites** (non-test), per repo — these are what "the look" means:
  - Sheets: `crates/ui-egui/src/app.rs:111` (in `setup_context_for_language`, called at
    construction), `crates/ui-egui/src/app/fonts.rs:81` and `:107`, `crates/ui-egui/src/app.rs:373`.
  - Docs: `crates/ui-egui/src/theme.rs:490`, `:1049`, `crates/ui-egui/src/window_close.rs:124`.
  - CAD: `crates/ui-egui/src/theme.rs:401`.
  - Decks: `crates/ui-egui/src/theme.rs:182` (`install_fonts_with`), `:279`, `:349`, `:392`, and
    `crates/ui-egui/src/app/frame.rs:5` (a comment about `set_fonts`/`set_visuals`).
  - PDF: `crates/ui-egui/src/app.rs:737` (comment), `crates/ui-egui/src/home.rs:291`/`:326` (tests),
    `crates/ui-egui/src/widgets.rs:729`, and more — count them yourself and list every NON-TEST one.
  **For each app, state whether `set_fonts`/`set_style`/`set_visuals`/`set_zoom_factor` is called
  only at construction, or also per frame from `logic`.** The host re-issues the active module's
  fonts after every module's `logic` has run, so a per-frame call is survivable — but it must be
  known.
- **`set_zoom_factor`**: Sheets `crates/ui-egui/src/app/prefs.rs:46`, Docs
  `crates/ui-egui/src/app.rs:286`, CAD `crates/ui-egui/src/lib.rs:858`, Decks
  `crates/ui-egui/src/app/frame.rs:9`, PDF `crates/ui-egui/src/platform/ui_scale.rs:69`.
- **Storage users**: only CAD (`cc.storage` ×3 — profile, prefs, autosave prefs) and PDF
  (`cc.storage` ×2 — settings, window state). Sheets, Docs and Decks ignore storage entirely.
- **`eframe::Frame::storage()` is public** (`pub fn storage(&self) -> Option<&dyn Storage>`), so a
  host can obtain storage at runtime — this is how the suite will get it for a lazily-built module.

## Deliverable

Write a report to `/opt/hermes-agent/workspace/office/PHASE2-<app>.md` and return the same content
as your final summary:

1. **VERDICT**: one line — "HOST API DONE" or "HOST API FAILED" (with the reason).
2. The exact gate command and its exit code.
3. The **real** signatures you added (not the sketch above).
4. Whether `set_fonts`/`set_style`/`set_visuals`/`set_zoom_factor` is called at construction only, or
   also per frame — with file:line for each non-test call site.
5. What stayed in `Shell::new` and why (the platform extras that need the real `CreationContext`).
6. Every obstacle, with the exact error text and how you resolved it.
7. Anything you could not do, and why.
8. The final `git status --porcelain` output.

Do not commit. Do not touch another repo. Do not implement Phase 2's suite code — only this API.


---

# PDF addendum (pdfcraft only)

PDF's `Shell::new` uses more of `CreationContext` than the other four. Enumerate what it needs and
where each use goes. Verified facts:

- `crates/pdf-shell/src/lib.rs:61` — `pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self`.
- **`cc.storage` — 2 uses, both load state, both must move to the `storage` parameter:**
  - line 71: `cc.storage.and_then(|s| s.get_string("iridium_pdf").or_else(|s| s.get_string(LEGACY_STORAGE_KEY)))` -> `app.restore(&json)`
  - line 74: `app.window_state = WindowState::started_with(cc.storage)`
- **`cc.egui_ctx` — used for the app's own wiring.** Pass the `ctx` parameter instead.
- **`cc.winit_window()` — Windows only, line 82.** Sets the taskbar icon. It needs the REAL window,
  which a host constructor does not have. Keep this in `Shell::new` (not `new_for_host`), exactly as
  today. Say so in your report.
- **`cc.wgpu_render_state` — lines 116-127.** Only a LOG line plus `notify_software_renderer(...)`,
  which decides whether to warn about a software renderer. `cc.wgpu_render_state` is
  `Option<egui_wgpu::RenderState>` — it cannot be synthesised. Keep the wgpu branch in `Shell::new`
  and have the host path simply log "renderer: OpenGL (glow)" — BUT check what
  `notify_software_renderer` does and whether skipping it changes behaviour. If it matters, the
  cleanest fix is to add the renderer info to `Launch` as an optional field, so the host can pass it
  through. Rule 5 of the brief allows adding a field to `Launch`. Decide and report.
- **`native_wayland(cc)` — line 227.** A helper that asks `cc.display_handle()` whether the display
  is Wayland. It is used only when `launch.os_events` is `Some(..)` (line 108). The `os_events`
  connect callback takes `(&egui::Context, bool)`. A host that does not set up OS events does not
  need this. Keep `native_wayland` usable from `Shell::new`; in the host path pass `false` — and
  report that choice.
- **The macOS native menu** is installed at line 113 via `app.install_native_menu(&cc.egui_ctx)`.
  `install_native_menu` must be able to re-install it later, so the `Shell` must keep whatever
  handle `install_native_menu` returns. READ `iridium_pdf_ui_egui::install_native_menu` and store
  what it returns, so `Shell::install_native_menu(&self)` can re-apply it.
- **`Launch` fields** (from the destructuring at line 62): files, options, control_file,
  create_images, combine, integrated, window_controls, two_paged, os_events, startup_error.
  `startup_error` is `Rc<RefCell<Option<String>>>` — shared with the host. Keep it that way.
- `startup_error` is set at line 111 when the control file cannot be published, and `Shell::new`
  returns early. Your `new_for_host` must keep that early-return behaviour.

- **`install_native_menu` may need `&mut self`.** PDF's `PdfApp::install_native_menu(&mut self, ctx)`
  (`crates/ui-egui/src/menus.rs:608`) stores the built menu in `self.native_menu` and returns a
  `bool`. If re-installing requires `&mut self`, make `Shell::install_native_menu` take `&mut self`
  and say so in your report. The suite calls it on module switch, where it has `&mut` access anyway.
- Check the other four apps' `crates/<app>-shell/src/native_menu.rs` for the same question: what
  does each one expose, and can a menu be re-installed after the fact, or only installed once?
  If an app's menu can only be installed once (it stores the handle inside the app), then
  `Shell::install_native_menu` must call the app's own install function and that function may need
  `&mut self`.

- **ONLY Sheets and CAD have a native menu.** Docs and Decks have NO `native_menu.rs` in their shell
  crate — they use in-window menus. Their `Shell::install_native_menu` must be a no-op (and must
  say so in its doc comment), not an invented mechanism.
- **The install signatures differ per app** — do not force them into one shape:
  - Sheets: `NativeMenu::install(ctx: &egui::Context) -> NativeMenu` (`crates/sheets-shell/src/native_menu.rs:223`).
    It RETURNS the menu, so the `Shell` can hold it and re-install it later. Sheets' menu also has
    `poll(&mut self, app: &mut SheetApp, ctx, viewport)` at line 317 — check whether the Shell
    already polls it, and keep that working.
  - CAD: `NativeMenu::install(app: &mut CadApp, ctx: &egui::Context) -> Self`
    (`crates/cad-shell/src/native_menu.rs:39`) — it takes `&mut CadApp` AND returns the menu.
    CAD's menu also has `poll(&mut self, app: &mut CadApp, ctx)` at line 117.
  - PDF: `PdfApp::install_native_menu(&mut self, ctx) -> bool` (`crates/ui-egui/src/menus.rs:608`) —
    it stores the menu INSIDE the app and returns a bool.
  So: Sheets can re-install by holding the returned `NativeMenu`; CAD must keep both the app and the
  menu; PDF must call the app's own method again. If `Shell::install_native_menu` needs `&mut self`
  for CAD or PDF, take `&mut self` and SAY SO — the suite calls it on module switch with `&mut`.
  The one uniform rule: the method must exist on every Shell and must be safe to call when the
  platform has no native menu (no-op).

---

# Cross-cutting findings from the other four (READ THIS — it changes the approach)

Three of the five apps install their look LAZILY, on the first frame, not at construction. If you
capture `ctx.fonts()` / `ctx.style()` at construction you will capture **egui's defaults**, not your
app's look. The other four agents solved it as follows — follow the same pattern:

- **`ctx.fonts()` PANICS before egui's first pass** ("No fonts available until first call to
  Context::run()"). Do NOT call it in a constructor. Capture the `FontDefinitions` your app BUILT
  instead (e.g. `theme::font_definitions(...)`), which is what Sheets, CAD and Decks did.
- **`ctx.style()` does not exist in egui 0.37.** Use `ctx.global_style().as_ref().clone()`.
  (All four agents hit `error[E0599]: no method named 'style' found for reference '&egui::Context'`.)
- **Sheets** installs its look at construction (`setup_context_for_language`) and again per frame.
  Its capture is genuine.
- **Docs** installs its look from `logic` on the first frame, so its capture is egui's defaults. It
  deliberately did NOT add a warm-up (running `logic` in the constructor would change the standalone
  app's behaviour). The suite must therefore run a module's `logic` after a switch rather than rely
  on the captured fonts.
- **CAD** installs its look from `ui_frame_begin` (called from `CadApp::ui`). It extracted the
  installs into a new public `CadApp::install_look(&mut self, ctx)` and calls it from both
  `new_for_host` (once, at construction) and `ui_frame_begin` (each frame). It also added the field
  `installed_fonts: Option<FontDefinitions>` and `theme::font_definitions(ctx, language)`. Note:
  making `theme::install_fonts` return `FontDefinitions` broke two tests
  (`error[E0308]: mismatched types ... expected '()', found 'FontDefinitions'` in
  `tests/it/properties_units.rs:21` and `shared_controls.rs:121`) — it added a separate
  `font_definitions` function instead.
- **Decks** installs its look from `logic` via `ui_setup` on the first frame. It does the three
  installs eagerly in `new_for_host` (zoom, `set_fonts(theme::font_definitions(&app.ui_fonts.faces))`,
  `theme::apply(Tokens::for_brightness(brightness))`) and captures after. Note its
  `ui_fonts.faces` is EMPTY at construction, so its captured font set is the base set only —
  fallback faces added later are not in it. Say so in your report if PDF is the same.

**Per-frame look calls that a host must know about** (the host re-issues the active module's look
after every module's `logic` has run, so these are survivable — but they must be KNOWN):
- Sheets: `set_fonts` per frame via `start_font_setup` (`app/fonts.rs:84`), pinned fonts
  (`app/fonts.rs:81`), workbook font rebind (`app/fonts.rs:107`); `set_visuals_of`/`set_theme`
  (`theme.rs:334`, `:337`); `set_theme` (`app/prefs.rs:24`); `set_zoom_factor` (`app/prefs.rs:46`).
- Docs: `theme::apply` (`app.rs:269`), `set_zoom_factor` (`app.rs:286`), `apply_animations`
  (`app.rs:293`), `set_fonts` via `app/fonts.rs:173` -> `theme.rs:490` and via the draw path
  `canvas.rs:1545` -> `theme.rs:490`.
- CAD: everything runs from `ui_frame_begin` (`ui-egui/src/lib.rs:794`); `set_fonts`
  (`theme.rs:401`), `set_visuals_of` (`theme.rs:792`, `:793`), `set_theme` (`:796`),
  `all_styles_mut` (`:803`), `animation.rs:58`; `set_zoom_factor` (`lib.rs:858`).
- Decks: `ui_setup` (`app.rs:286`, defined in `app/frame.rs`); `set_zoom_factor` per frame
  (`app/frame.rs:9`); `set_fonts` (`theme.rs:182` from `app/frame.rs:12`, `uifonts.rs:130` every
  frame from `app/ime.rs:30`, `presenter.rs:121`); `set_visuals` + `global_style_mut`
  (`theme.rs:161`, `:162`).

**All four apps change their look at runtime** (language switch, workbook fonts, brightness, theme
preference, UI scale). A captured look therefore goes stale — the host re-issues the captured set on
switch, and the app reinstalls its own on the next frame. Say which applies to PDF.
