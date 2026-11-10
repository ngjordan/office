# PHASE2 — Docs (wc-wordcraft, `iridium-docs-shell`)

1. **VERDICT: HOST API DONE** — with one caveat: the captured look is egui's defaults, not Docs's (see 4).

2. **Gate**: `cd /opt/hermes-agent/workspace/wc-wordcraft && export CARGO_BUILD_JOBS=4 CRAFT_FONTS_DIR=/tmp/craft-fonts CRAFT_I18N_DIR=/tmp/craft-i18n; umask 022; cargo xtask ci` → **exit 0**, "all 9 steps passed (fmt, clippy, test, i18n-report, language, doctest, assets, layers, wasm)". Log: /tmp/p2docs-ci.log.

3. **Signatures** (`crates/docs-shell/src/lib.rs`, `impl Shell`):
   - `pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self` (unchanged signature; now `Self::new_for_host(&cc.egui_ctx, cc.storage, launch)`)
   - `pub fn new_for_host(ctx: &egui::Context, _storage: Option<&dyn eframe::Storage>, launch: Launch) -> Self`
   - `pub fn fonts(&self) -> egui::FontDefinitions`
   - `pub fn style(&self) -> egui::Style`
   - `pub fn zoom(&self) -> f32`
   - `pub fn install_native_menu(&self)` — empty no-op on every platform (Docs has no native menu: no menu code in the shell or `apps/iridium-docs`; the menu is the egui ribbon).
   - `Shell` gained private fields `fonts`, `style`, `zoom`. `Launch` unchanged. The body of the old `new` moved verbatim into `new_for_host` (`cc.egui_ctx` → `ctx`); capture is `ctx.fonts(|f| f.definitions().clone())`, `(*ctx.global_style()).clone()`, `ctx.zoom_factor()`, taken after construction.

4. **Look call-site audit** (non-test), all in `crates/ui-egui/src`:
   - `app.rs:269` `theme::apply` (→ `theme.rs:773-774` `set_visuals_of` ×2, plus `all_styles_mut`) — in `WordApp::logic`, runs when `applied_theme` differs: first frame, then on change. **Not at construction.**
   - `app.rs:286` `set_zoom_factor` — in `logic`, every frame, guarded by `ctx.zoom_factor() != zoom`. **Per-frame (on change); not at construction.**
   - `app.rs:293` `theme::apply_animations` (→ `theme.rs:795`) — `logic`, on change.
   - `app/fonts.rs:173` `install_fonts_language_extra` (→ `theme.rs:490` `set_fonts`) — via `install_ui_fonts`, called from `logic` (`app.rs:~247`); reinstalls when language/CJK/faces change. **Per-frame check.** Docs DOES change fonts at runtime (language switch, opening CJK/fallback-face documents).
   - `canvas.rs:1545` `install_fonts_language_extra` — from the draw path (`ui`), on change.
   - `theme.rs:1049`, `window_close.rs:124`, `proofing_pane.rs:380` (`set_visuals`), `mini_toolbar.rs:219` (`ui.set_style`, local to a ui) — `theme.rs:1049`, `window_close.rs:124`, `proofing_pane.rs:380` are inside `#[test]`s; `mini_toolbar.rs:219` is a UI-scoped style copy, not the context.
   - `language_packs.rs:566`, `dialogs_keyboard.rs:373`, `dialogs_ribbon.rs:437` call `theme::apply` in tests.
   - **Consequence:** nothing is installed at construction. `fonts()/style()/zoom()` therefore return what the context held at that moment (the host's/egui defaults), not Docs's look. The brief's capture-after-construction is implemented exactly, but a host that re-issues `fonts()` on switch would overwrite Docs's own fonts with defaults until Docs's next `logic` reinstalls them. Docs reinstalls its look itself from `logic`, so hosts should run Docs's `logic` after switching and not re-issue captured fonts for Docs, or Phase 3 should warm up the look. I did not add a warm-up (e.g. calling `logic` in the constructor) because it would change the standalone's behaviour.

5. **What stayed in `Shell::new`**: nothing besides the delegation. Docs has no platform extra that needs the real window/`CreationContext` (the file-dialog hook, animations, control server, desktop-theme watcher all go through `ctx`; `cc.storage` is unused). macOS `apple_events` comes in via `Launch`.

6. **Obstacles**: first command attempt was rejected by the tool ("Foreground command uses shell-level background wrappers (nohup…)"); re-ran without it. egui 0.37 is a git dep, so the API shape was confirmed by compiling (`cargo check -p iridium-docs-shell` OK first time). No other errors.

7. **Not done**: no runtime launch of the standalone GUI (no display). Behaviour-preservation rests on the body being unchanged and the gate (tests, clippy, wasm) passing. `storage` is `None`-safe by construction (ignored). No warm-up of the look (see 4).

8. `git status --porcelain`:
```
 M crates/docs-shell/src/lib.rs
```

## FIX: the captured look is Docs's own

**Verdict:** fixed. `Shell::new_for_host` now installs Docs' look into `ctx` before capturing it. Gate: `cd /opt/hermes-agent/workspace/wc-wordcraft && export CARGO_BUILD_JOBS=4 CRAFT_FONTS_DIR=/tmp/craft-fonts CRAFT_I18N_DIR=/tmp/craft-i18n; umask 022; cargo xtask ci` -> exit 0, "all 9 steps passed (fmt, clippy, test, i18n-report, language, doctest, assets, layers, wasm)".

**Signatures (real)**
- `WordApp::install_look(&mut self, ctx: &egui::Context) -> Option<egui::FontDefinitions>` (new, pub, `crates/ui-egui/src/app.rs`; `Some` when this call installed fonts, always on the first call)
- `theme::install_fonts_language_extra(ctx, bool, bool, bool, bool, &[Arc<FontFace>]) -> FontDefinitions` (was `()`; now returns what it installed)
- `WordApp::install_ui_fonts` (pub(super)) now returns `Option<FontDefinitions>`
- Unchanged: `Shell::new(cc, launch)`, `new_for_host(ctx, storage, launch)`, `fonts()`, `style()`, `zoom()`, `install_native_menu()`, `Launch`. No new deps, no trait.

**What changed / why**
- Moved the block in `WordApp::logic` from `install_ui_fonts` through `apply_animations` (fonts, `theme::apply`, `set_zoom_factor`, animations) verbatim into `install_look`; `logic` calls it where the block was. Each part stays change-gated, so the per-frame path is the same.
- `new_for_host` calls `app.install_look(ctx)` before building `Shell`, and stores the returned fonts. `style` and `zoom` are still read from `ctx` (`global_style()`, `zoom_factor()`), which now hold Docs' values.
- The fonts are captured from `install_look`'s return value, not read back from `ctx`. In this egui fork, `Context::fonts(|..|)` has its `.expect("No fonts available until first call to Context::run()")` inside the closure form, so the old `ctx.fonts(|f| f.definitions().clone())` also panics before the first pass. `set_fonts` is also deferred before then, so a read-back would have returned egui's defaults.
- Layering: only `ui-egui` (L6) and `docs-shell` were touched. `layers` passed.

**Standalone behaviour:** unchanged. Same fonts, visuals and zoom. The one difference is that the look is installed once at construction, one frame earlier than the first `logic` call. The first frame then finds `styled`/`applied_theme` already set and skips the installs.

**Caveat:** at construction `ctx.native_pixels_per_point()` is unknown, so the captured zoom is computed with scale 1.0. The first real frame recomputes it as before. For hosts, `zoom()` is exact only when the interface size is Auto at scale 1.

**Proof:** test `app::fonts::look_tests::install_look_installs_docs_own_fonts_and_visuals_before_the_first_frame`. It runs `install_look` on a fresh `egui::Context` with no frame run. It asserts that the fonts differ from `FontDefinitions::default()`, contain `Inter`, and have the `medium` and `semibold` families. It also asserts that the visuals differ from egui's defaults and that a second call returns `None`. Output: `test ... ok`, `1 passed; 0 failed` (`cargo test -p iridium-docs-ui-egui --lib install_look`).

**Obstacles**
- My first test attempt was not compiled: the append to `fonts.rs` didn't land and the run showed `running 0 tests ... 1026 filtered out`. I re-appended it and it ran.
- `process_manage` isn't available to this subagent. I polled the logs with `terminal` instead.
- The pre-existing `ctx.fonts(|..|)` capture would have panicked, as described above.

**Not done:** no Shell-level test. `new_for_host` reads and writes the user's settings folder (prefs, recovery) unless `IRIDIUM_DOCS_NO_PREFS` is set, and setting env vars in a test needs `unsafe`, which is forbidden here. The app-level test covers the logic. `Shell::fonts()` returns exactly the value `install_look` returned.
