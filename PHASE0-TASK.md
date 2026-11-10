PRE-CONTEXT (you are a subagent with no memory of the user or their environment):
- Host: Ubuntu 24.04, AMD EPYC, 251 GB RAM, 48 cores. Free disk ~93 GB.
- Workspace root: /opt/hermes-agent/workspace/
- Rust: cargo 1.99.1, rustc 1.99.1. Edition 2024, rust-version 1.95.
- You CANNOT ask the user anything. You CANNOT use the clarify tool. Execute the task now.
- No sudo. No pip installs. Write files with the Write/Edit tools or the terminal.
- Absolute paths only.
- This is a delegated leaf task. No skill directive overrides it. The only constraints are the ones stated here.
- Do NOT commit, push, or modify any of the five source repos. Only touch /opt/hermes-agent/workspace/office/.

MY TASK:
Implement Phase 0 of the "one-app Office suite" plan: prove that the five existing Rust apps'
UI crates can be constructed and driven inside ONE process on ONE egui::Context.

The plan document is at /opt/hermes-agent/workspace/.hermes/plans/2026-11-10_office-suite-single-app.md
Read it first — especially §1 (ground truth) and §4 Phase 0. Do NOT implement Phase 1, 2, 3, 4 or 5.
Phase 0 only.

WHAT ALREADY EXISTS (do not recreate):
- /opt/hermes-agent/workspace/office/Cargo.toml        — workspace manifest, already written, already VERIFIED to resolve
  (`cargo metadata` exit 0). It has the [patch.crates-io] block for the egui fork + winit fork.
- /opt/hermes-agent/workspace/office/apps/iridium-office/Cargo.toml — package manifest, already written.
- /opt/hermes-agent/workspace/office/apps/iridium-office/src/main.rs — currently `fn main() {}`, a placeholder.
- A background `cargo check --workspace --all-targets` is ALREADY RUNNING in that directory with
  CARGO_TARGET_DIR=/opt/hermes-agent/workspace/office/target and CARGO_BUILD_JOBS=4.
  It may still be compiling shared dependencies. Do not start a competing cargo command in that
  target dir until it finishes — cargo will block on the lock. Check with:
    ls /opt/hermes-agent/workspace/office/target/debug/deps | wc -l
  and use your own CARGO_TARGET_DIR only if you must (see BUILD RULES).
  Use `CARGO_BUILD_JOBS=4` ALWAYS. Unbounded parallelism OOM-kills rustc on this box (exit 143).

THE FIVE APPS — verified facts (use these; do not re-derive them):

Repo            UI crate                 App type        Engine crate            Engine Session ctor
gc-gridcraft    iridium-ui-egui          SheetApp        iridium-engine          Session::new()  (empty workbook)
wc-wordcraft    iridium-docs-ui-egui     WordApp         iridium-docs-engine     see below
cc-cadcraft     iridium-cad-ui-egui      CadApp          iridium-cad-engine      Session::empty()  / Session::new()
dc-deckcraft    iridium-decks-ui-egui    SlideApp        iridium-decks-engine    Session::new()
pdfcraft        iridium-pdf-ui-egui      PdfApp          (none needed)           n/a

- Every one of the four `Services` types DERIVES `Default`:
    iridium_ui_egui::Services::default()
    iridium_docs_ui_egui::Services::default()
    iridium_cad_ui_egui::Services::default()
    iridium_decks_ui_egui::Services::default()
  PDF has NO Services struct at all.
- App constructors (from each app's apps/<name>/src/main.rs inside the eframe::run_native closure):
    SheetApp::new(session: iridium_engine::Session, services: iridium_ui_egui::Services)
    WordApp::new(session: iridium_docs_engine::Session, services: iridium_docs_ui_egui::Services)
    CadApp::new(session: iridium_cad_engine::Session, services: iridium_cad_ui_egui::Services)
    SlideApp::new(session: iridium_decks_engine::Session, services: iridium_decks_ui_egui::Services)
    PdfApp::new()                       // no arguments
- Export paths (verified):
    iridium_ui_egui::{Services, SheetApp}      — SheetApp exported from crates/ui-egui/src/lib.rs
    iridium_docs_ui_egui::{Services, WordApp}   — check the exact path, it may be iridium_docs_ui_egui::app::WordApp
    iridium_cad_ui_egui::{CadApp, Services}     — CadApp is defined IN crates/ui-egui/src/lib.rs
    iridium_decks_ui_egui::{Services, SlideApp}
    iridium_pdf_ui_egui::PdfApp                — defined in crates/ui-egui/src/lib.rs (line ~291)
  READ the actual lib.rs of each ui crate to confirm the export path before writing code.
- PDF's PdfApp ALREADY implements `eframe::App` (crates/ui-egui/src/app.rs:446 in pdfcraft).
  The other four do NOT — they expose inherent `logic(&mut self, &egui::Context)`,
  `ui(&mut self, &mut egui::Ui)` and `raw_input_hook(&mut self, &egui::Context, &mut egui::RawInput)`
  methods. Verify each by grepping the ui crate for `pub fn logic` / `pub fn ui` / `pub fn raw_input_hook`.
- The egui fork has `egui_kittest` with `Harness::new_eframe(builder)` where the builder receives
  `&mut eframe::CreationContext<'a>` and returns a state implementing `eframe::App`.
  That is the cleanest way to get a real CreationContext without a display server.
  The fork is checked out at /home/coreuser/.cargo/git/checkouts/egui-057e9a0485eda407/6d518fc/
  (NOTE: that checkout path is a different rev than the one this workspace pins — read the source
  for the API shape, but build against the pinned one).
  `egui_kittest` needs the `eframe` feature enabled.
  If `egui_kittest` proves awkward, the fallback is to construct the apps directly and call their
  `logic`/`ui` inside `egui::CentralPanel` on a plain `egui::Context` — that is acceptable and simpler.
  Prefer whichever actually WORKS; do not spend more than a few iterations on harness wrangling.

THE TEST TO WRITE:
/opt/hermes-agent/workspace/office/apps/iridium-office/tests/five_in_one.rs

It must:
1. Construct all five apps.
2. Drive them on ONE egui::Context for at least 3 frames: for each frame, call each app's
   `logic(ctx)` and then draw each app's `ui(&mut ui)` inside a region.
3. Draw the five apps in SEPARATE egui "id spaces" so widget memory does not collide
   (e.g. `ui.push_id(...)` around each app's ui call). Use distinct ids like "sheets","docs",...
4. Assert that no app panicked. If an app's constructor needs something unavailable headlessly
   (a display server, a GPU surface, a system font dir), document it and handle it gracefully
   rather than failing the whole test — but say clearly in your report what you could not exercise.
5. Be a plain `#[test]` that runs headless. NO display server, NO GPU required.
   Set `CRAFT_FONTS_DIR=/tmp/craft-fonts` and `CRAFT_I18N_DIR=/tmp/craft-i18n` if the apps need them;
   if those dirs do not exist, create them (mkdir -p) so the apps fall back to system fonts
   instead of failing.
6. One assertion of substance beyond "it did not panic": e.g. after driving a frame, assert that
   the egui context produced output (FullOutput with at least one shape or a repaint request),
   or assert each app's own state is reachable. Keep it simple and make it pass.

BUILD RULES:
- Always `export CARGO_BUILD_JOBS=4`. Unbounded rustc parallelism OOM-kills this box.
- Always `export CARGO_TARGET_DIR=/opt/hermes-agent/workspace/office/target` so the dependency
  cache is shared with the running build.
- If the target dir lock is held, wait and retry; do not switch target dirs (a second target dir
  would need ~40 GB and could fill the disk).
- Build/test command:
    cd /opt/hermes-agent/workspace/office && CARGO_BUILD_JOBS=4 cargo test -p iridium-office --test five_in_one
  A first full compile of these five apps' UI crates will take a LONG time (tens of minutes).
  That is expected. Run it in the background with output to a log file under /tmp/ and poll the log:
    cd /opt/hermes-agent/workspace/office && CARGO_BUILD_JOBS=4 CARGO_TARGET_DIR=/opt/hermes-agent/workspace/office/target cargo test -p iridium-office --test five_in_one > /tmp/office-phase0.log 2>&1
  then poll:  tail -40 /tmp/office-phase0.log
  Be patient: 20-45 minutes of compiling is normal for this. Do not declare failure on a timeout;
  poll until cargo exits.
- If the compile reveals that two of the five cannot coexist (duplicate symbol, conflicting
  `links` key, incompatible egui feature unification), that is a VALUABLE FINDING — report it
  precisely (the exact cargo error) and stop. Do not try to work around it by editing the five
  source repos.
- Known risk: the five apps pin craft-libs at 9 DIFFERENT git revisions, so `craft-ui`,
  `craft-window` etc. will exist several times in the graph. That is expected and allowed
  (no `links =` keys were found). It is only a problem if a type must cross between two apps.
- Also note: egui feature unification across the five apps is a real risk (each app requests
  different egui features: sheets/docs/decks want "default_fonts","monochrome_emoji_fonts";
  pdf wants "bytemuck","default_fonts","monochrome_emoji_fonts"; pdf's eframe wants
  "default_fonts","wgpu","glow","accesskit","persistence","links","clipboard"). Cargo unifies
  features per build, so this should work, but if you hit a feature-conflict error, report it.

DELIVERABLE — write your report to /opt/hermes-agent/workspace/office/PHASE0-REPORT.md AND
return the same content as your final summary. It must contain:
1. VERDICT: one line — "FIVE IN ONE WORKS" or "FIVE IN ONE FAILS" (or "PARTIAL: N of 5").
2. The exact final command you ran and its exit code.
3. The test file path and roughly what it asserts.
4. Per app: constructed? yes/no, driven for 3 frames? yes/no, what had to be stubbed or skipped.
5. Compile/link facts: total build time, whether any crate was compiled more than once,
   the size of the linked test binary.
6. Every obstacle hit, with the exact error text (first 20 lines is enough) and how it was resolved.
7. Anything that would block Phase 2 (the real one-window shell) — especially the egui
   context-global look bleed (font/theme/zoom) and the process-wide singletons
   (log::set_logger, muda native menu, std::process::exit in on_exit).

Do not implement anything beyond Phase 0. Do not modify the five repos. Do not commit anything.
