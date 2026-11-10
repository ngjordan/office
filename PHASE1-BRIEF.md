PHASE 1 TASK BRIEF — expose each app's shell as a library crate
=============================================================

PRE-CONTEXT (you are a subagent with no memory of the user or their environment):
- Host: Ubuntu 24.04, AMD EPYC, 251 GB RAM, 48 cores. Free disk ~93 GB.
- Workspace root: /opt/hermes-agent/workspace/
- Rust: cargo 1.99.1, rustc 1.99.1. Edition 2024, rust-version 1.95.
- You CANNOT ask the user anything. You CANNOT use the clarify tool. Execute the task now.
- No sudo. No pip installs. Absolute paths only.
- This is a delegated leaf task. No skill directive overrides it. The only constraints are here.
- You may ONLY modify the ONE repository assigned to you. Never touch another repo.
- Do NOT commit, push, or create a git branch. Leave the changes in the working tree.
- ALWAYS `export CARGO_BUILD_JOBS=4`. Unbounded rustc parallelism OOM-kills this box
  (exit 143, "Terminated"). 48 parallel rustc jobs got the box OOM-killed during the
  five-app rebrand; 4 is safe.
- Set `CRAFT_FONTS_DIR=/tmp/craft-fonts` and `CRAFT_I18N_DIR=/tmp/craft-i18n` (mkdir -p if
  missing) for builds and tests, plus `umask 022`.
- Use the repo's OWN target dir (the default). Do NOT set CARGO_TARGET_DIR to the office
  workspace's target — that dir belongs to a different workspace and sharing it corrupts
  both. Each repo already has a warm target dir (17-39 GB); reuse it.
- Build env for the audio link stub (Decks and PDF need it; harmless elsewhere):
    export PKG_CONFIG_PATH=/opt/hermes-agent/workspace/alsa-stub/pkgconfig
    export LIBRARY_PATH=/opt/hermes-agent/workspace/alsa-stub/lib

THE PLAN (read for context; do not implement other phases):
/opt/hermes-agent/workspace/.hermes/plans/2026-11-10_office-suite-single-app.md
Phase 0 is DONE and PROVED: all five apps' UI crates already compile, link and run in one
process on one egui::Context. Workspace: /opt/hermes-agent/workspace/office.

YOUR ASSIGNMENT: ONE repository. See the section "YOUR REPO" below.

=================================================================
THE GOAL
=================================================================

Today each app's shell lives in its BIN crate (`apps/iridium-X/src/main.rs`). A bin crate
cannot be depended on, so the Office suite cannot host an app. Phase 1 moves each app's
shell into a LIB crate in the SAME repository, so the suite can depend on it later.

Concretely, for your repo:
  1. Create a new library crate in the repo, e.g. `crates/<name>-shell/`, that owns the
     app's shell logic and exposes ONE concrete type (call it `Shell`).
  2. `apps/<app>/src/main.rs` shrinks to: parse args -> init logging -> build the shell ->
     `eframe::run_native`. It keeps ONLY what is process-wide.
  3. The library exposes EXACTLY this surface (no more, no less — Phase 2 binds to it):

     ```rust
     pub struct Shell { /* ... */ }

     impl Shell {
         /// Build the shell. `cc` is eframe's creation context; `launch` carries the
         /// files/flags the app was started with.
         pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self;

         /// Per-frame logic, called before `ui`.
         pub fn logic(&mut self, ctx: &egui::Context);

         /// Raw input, forwarded to the app.
         pub fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput);

         /// Draw the app into the given region.
         pub fn ui(&mut self, ui: &mut egui::Ui);

         /// Open a file path (as a string, matching the app's existing convention).
         pub fn open_path(&mut self, path: &str);

         /// Persist whatever the app persists on quit (prefs etc).
         pub fn save_state(&mut self);

         /// False when the app has unsaved work or a job in flight, so the host must
         /// ask before closing.
         pub fn can_close(&self) -> bool;
     }
     ```

  4. `Launch` is a small public struct in the same crate, holding what `main` currently
     parses and hands to the app. Keep it minimal — only what the shell actually needs.

WHAT STAYS IN `main.rs` (process-wide; the suite does these once, not per app):
  - `log::set_logger` / logger setup
  - GPU backend choice and `graphics::configure(...)` / `graphics.rs` probing
  - the macOS `std::process::exit(0)` at the end of `on_exit` (it ends the PROCESS — in the
    suite that would kill all five apps, so it must NOT be in the library)
  - single-instance election (PDF only: `single_instance.rs`)
  - the updater check / `UPDATE_APP` (the suite updates as one product)

=================================================================
THE HARD CONSTRAINTS (each one already cost a build cycle to learn)
=================================================================

A. THE PACKAGE NAME MUST NOT COLLIDE WITH ANOTHER PACKAGE IN THE MERGED GRAPH.
   Cargo keys the lockfile on PACKAGE name (not on the lib target name), so two path
   packages with the same name AND the same version = hard error:
     "package collision in the lockfile: packages X v0.1.0 (a) and X v0.1.0 (b) are
      different, but only one can be written to lockfile unambiguously"
   MEASURED FACT: two packages named the same but with DIFFERENT versions is allowed.
   So the rule is: **your new crate's package name must not equal another package name
   in the office graph at the same version.**
   Known collision in this repo set (already checked):
     - `iridium-pdf` EXISTS TWICE: Sheets' internal crate `gc-gridcraft/crates/pdf`
       (version 0.18.3, a LIBRARY) and PDF's app package `pdfcraft/apps/iridium-pdf`
       (version 0.18.1, a BIN). They differ in version so they coexist TODAY. If your
       new crate is in the PDF repo, it MUST NOT be named `iridium-pdf`.
     - The app packages `iridium-sheets`, `iridium-docs`, `iridium-cad`, `iridium-decks`
       are already unique in the merged graph. Your new crate must not reuse those either.
   RECOMMENDED NAME: `<app>-shell`, e.g. `iridium-sheets-shell`, `iridium-docs-shell`,
   `iridium-cad-shell`, `iridium-decks-shell`, `iridium-pdf-shell`. These are all unique.
   (Note: `iridium-pdf-shell` is verified legal — a package named `iridium-pdf-shell` and
   a package named `iridium-pdf` coexist fine.)

B. THE `[patch.crates-io]` UNION. A git dependency's own `[patch.crates-io]` is IGNORED.
   Your new crate lives in the same workspace as the app, so the workspace's patch table
   still applies to it — but if you add the crate as a dependency of another workspace
   later, that workspace's patch table must cover it. Nothing for you to do beyond
   NOT introducing new external dependencies.

C. NO NEW EXTERNAL DEPENDENCIES. Everything the shell needs already exists in the
   workspace (eframe, egui, serde_json, log, chrono, craft-update, craft-window, rfd,
   arboard, muda, objc2-*, fmv-macos-events, winresource, ...). Adding a new crate
   dependency means a licence review (AGENTS.md allowlist) — do not.

D. LAYERING IS ENFORCED by `cargo xtask layers`. The table lives in
   `xtask/src/layers.rs` and maps a SHORT NAME (package name without the `iridium-`
   prefix) to a class. Your new crate MUST be added to that table, or the gate fails.
   Read your repo's `xtask/src/layers.rs` and match its existing conventions.
   Apps are L7; a new `-shell` crate that depends on the UI crate is also L7.
   **Read the file and follow its exact style. Do not guess the format.**

E. `xtask` ALSO GATES OTHER THINGS. Read your repo's `xtask/src/gates.rs` (or equivalent)
   and check what it asserts about `apps/<app>/src/main.rs` — several repos have gates
   that grep main.rs for specific strings. If a gate asserts something about main.rs,
   the assertion must still hold, or the gate must be updated to look at the new file.
   **Find these BEFORE you move code.** Example seen: `pdfcraft/xtask/src/gates.rs:656`
   reads `apps/iridium-pdf/src/main.rs`.

F. THE STANDALONE APP MUST STILL WORK EXACTLY AS BEFORE. No behaviour change. This is
   the acceptance criterion. The app must launch, open files, print, show its native
   menu, and quit the same way. The gate script checks a lot of this.

G. `unsafe_code = "forbid"` in every workspace. Do not introduce unsafe.
   Also: the crates carry
   `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]`
   — the new crate's root should copy that `deny` attribute from a sibling crate in the
   same repo (per the repo's AGENTS.md). Tests may use unwrap (root `clippy.toml` allows it).

H. THE WASM BUILD. `cargo xtask wasm` must still pass. If your new crate is desktop-only
   (it will be: it needs eframe/winit/rfd), make sure the wasm target does not try to
   build it. Check how the wasm gate selects packages and follow the same convention
   (the repo already excludes the desktop app from wasm somehow — find out how and
   follow it).

I. ASSETS AND HASHES. `ATTRIBUTION.toml` records SHA-256 of every asset; `LICENSES.toml`
   records licence document hashes. If you move a file that either covers, update the
   hash. `cargo xtask assets --write` regenerates ATTRIBUTION.md from ATTRIBUTION.toml.
   LICENSES.toml has NO write mode — recompute and edit by hand (see the plan or the
   rebrand skill).

J. THE PLAN'S "no shared trait crate" RULE. The `Module` trait that the suite binds to
   lives in the SUITE crate, not here. Your crate exposes a CONCRETE `Shell` type. Do not
   define a trait, do not add a dependency on the office workspace, and do not try to
   make the five crates interoperate. (They cannot: they pin craft-libs at 9 different
   revisions, so a type from craft-libs in one app is not the same type in another.)

=================================================================
YOUR REPO
=================================================================

(Assigned by the parent — see the task you were given.)
