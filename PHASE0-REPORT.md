# Phase 0 report

**VERDICT: FIVE IN ONE WORKS**

## Command
`cd /opt/hermes-agent/workspace/office && CRAFT_FONTS_DIR=/tmp/craft-fonts CRAFT_I18N_DIR=/tmp/craft-i18n PKG_CONFIG_PATH=/opt/hermes-agent/workspace/alsa-stub/pkgconfig LIBRARY_PATH=/opt/hermes-agent/workspace/alsa-stub/lib CARGO_BUILD_JOBS=4 CARGO_TARGET_DIR=/opt/hermes-agent/workspace/office/target cargo test -p iridium-office --test five_in_one -- --nocapture`
→ exit 0, `1 passed`, 2.27 s run.

## Test
`apps/iridium-office/tests/five_in_one.rs`. One `egui::Context`; builds all five apps (Sheets, Docs with `sample_document()`, CAD `Session::empty()`, Decks, PDF `PdfApp::new()`) with `Services::default()`. 3 frames via `ctx.run_ui`: each app's `raw_input_hook`, then `logic`, then each `ui` inside `push_id("sheets"|"docs"|"cad"|"decks"|"pdf")` in a `CentralPanel`. PDF is driven through `eframe::App` with `eframe::Frame::_new_kittest()`. Asserts total shapes > 0: frames painted 1031 / 1563 / 1565 shapes. No panic.

## Per app
All five: constructed yes, driven 3 frames yes. Nothing stubbed except `Services::default()` (no file dialogs, no host hooks, no update check, no prefs load). `egui_kittest` was not needed. Not exercised: real window/GPU, input events, prefs persistence, file open, native menu, control server. All of those live in each app's `main.rs`.

## Compile/link
- Test binary: 2.97 GB (debug, with debuginfo). Test-profile compile of the final crate: 8 s once deps were built.
- Wall time from first test build to green was ~325 s for the last cycle. The dependency compile was spread over earlier cycles and the parent's builds, so there is no single total.
- The craft-libs revisions do appear several times in the graph (expected, allowed). No `links` conflict. No egui feature-unification error.

## Obstacles
1. **hayro-jbig2 0.3.1 from crates.io vs vendored 0.3.0.** `cannot find function charge_reading in crate hayro_jbig2`. Fixed by the parent: patch the vendored pdfcraft crates.
2. **Stock krilla 0.8.2 instead of the craft-libs fork.** 8 errors in craft-pdf, e.g. `unresolved imports GroupColorSpace/OutputIntent/Trapped`, `CRAFT_UNWIND_POISONING` missing. Fixed by the parent: patch krilla from craft-libs rev 92d392c.
3. **No test target at first.** The test file was not yet written. I wrote it.
4. **API drift in the egui fork.** `Context::run` is now `run_ui(raw, |root_ui|)`, and `CentralPanel::show` takes `&mut Ui`. Fixed in the test.
5. **Panic `Dropped TexturesDelta with 40 unapplied deltas`.** Headless, nothing applies the textures. Fixed with `out.textures_delta.clear()`.

## Findings for Phase 2
- **The office `[patch.crates-io]` must be the union of all five repos' patch tables.** Git dependencies' own patch tables are ignored. cc-cadcraft and dc-deckcraft patch krilla/hayro from other craft-libs revs (e9c3f87, 04428cee). This was not hit, because the test compiles against the 92d392c fork. It is a latent risk for Phase 5 (git+rev deps).
- **Global look bleed is real and unexercised.** All five apps ran on one `Context` and did not panic, but each app's `logic`/`ui` presumably sets fonts, theme and zoom on the shared context. Whichever ran last wins. Phase 2 needs to re-apply the active module's style on every tab switch (or per-module style scoping). I did not measure this.
- **Process-wide singletons live in each `main.rs`.** `log::set_logger`, the muda native menu, and `std::process::exit(0)` in `on_exit` were not touched by this test and are not in the libs. Phase 1 has to keep them out of the lib shell, per the plan.
- **Build env.** The audio link stub env (`PKG_CONFIG_PATH`, `LIBRARY_PATH`) is required. The binary is ~3 GB in debug, so use release or `strip` for shipping.
- The `egui_demo_lib`, `egui_inspection`, `egui_kittest` and `egui_system_font_provider` patch entries and the hayro-cmap/jbig2/postscript patches log "not used" warnings. They are harmless.

## Files
Created: `apps/iridium-office/tests/five_in_one.rs`, `PHASE0-REPORT.md`. The five source repos are untouched.
