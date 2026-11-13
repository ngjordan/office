# Adding PhotoCraft, FilmCraft and LightCraft to the Office suite

Date: 2026-11-13. Three new apps, same treatment Mail got.

## What the suite is

`/opt/hermes-agent/workspace/office` — one window, one egui context, several apps.
It currently hosts six: Sheets, CAD, Docs, Decks, PDF, Mail, plus a Browser being built.
The visible product name is **Workspace**.

## The contract every app must satisfy

The suite's host API is `apps/iridium-office/src/module.rs`. Every app needs a
**shell crate** that exposes exactly these signatures. Copy `mc-mailcraft`'s
`crates/mail-shell/` — it is the reference implementation, written for this exact
contract.

```rust
pub struct Shell { /* ... */ }
pub struct Launch { /* ... */ }

impl Shell {
    pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self;
    pub fn new_for_host(ctx: &egui::Context, storage: Option<&dyn eframe::Storage>, launch: Launch) -> Self;
    pub fn logic(&mut self, ctx: &egui::Context);
    pub fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput);
    pub fn ui(&mut self, ui: &mut egui::Ui);
    pub fn backdrop(&self) -> [f32; 4];
    pub fn open_path(&mut self, path: &str);
    pub fn save_state(&mut self);
    pub fn can_close(&self) -> bool;
    pub fn zoom(&self) -> f32;
    pub fn fonts(&self) -> egui::FontDefinitions;
    pub fn install_native_menu(&mut self);
}
```

Read `mc-mailcraft/crates/mail-shell/src/lib.rs` before writing anything. It is
286 lines and shows every pattern: the `new_for_host` split, the look install,
the `Launch` struct, the zoom, the fonts.

**The rules that matter:**

1. **`new_for_host` is the host-facing constructor.** `eframe::CreationContext`
   holds a borrow and cannot be stored, so a host cannot build a module later.
   `new(cc, launch)` must become a thin wrapper that keeps only what genuinely
   needs the real context (a native window handle, a wgpu render state).

2. **`new_for_host` must install the app's look.** Most Craft apps install fonts
   and style LAZILY on the first frame. Capturing the look at the end of
   `new_for_host` would record egui's defaults, and a change-gated reinstall
   never fires again — so the module shows the wrong look forever. Extract the
   look install into one function, call it from BOTH the per-frame path and
   `new_for_host`, then capture.

3. **`set_fonts` REPLACES, it does not merge.** The host captures each module's
   look and re-issues it on switch. Do not swap font sets around chrome widgets.

4. **`ctx.fonts()` without a closure PANICS** before egui's first pass. Use the
   closure form: `ctx.fonts(|f| f.definitions().clone())`.

5. **Do not call `std::process::exit(0)` on quit.** That kills every other app in
   the suite. Expose `save_state()` and let the host decide.

6. **`can_close()` must derive from the app's EXISTING dirty state.** Never add a
   parallel dirty-tracking mechanism. If the app has no unsaved-changes concept,
   say so rather than inventing one.

7. **`install_native_menu` takes `&mut self`** where the app demands it; apps with
   no native menu get a documented no-op.

## Build environment (this box)

```bash
export CARGO_BUILD_JOBS=4          # unbounded parallelism OOM-kills this box
export CRAFT_FONTS_DIR=/tmp/craft-fonts
export CRAFT_I18N_DIR=/tmp/craft-i18n
export PKG_CONFIG_PATH=/opt/hermes-agent/workspace/alsa-stub/pkgconfig
export LIBRARY_PATH=/opt/hermes-agent/workspace/alsa-stub/lib:$LIBRARY_PATH
export LD_LIBRARY_PATH=/opt/hermes-agent/workspace/alsa-stub/lib
umask 022
```

**Disk is the real constraint.** The box was at 100% full. Build caches were
cleared; 211 GB is free. A full `cargo build` of one of these apps needs
40–60 GB. **Build one app at a time and clean its `target/` between apps.**
Run the tests with `cargo test -p <shell> -j 1` — the test binaries are ~2.8 GB
each and parallel test links will fill the disk.

The Craft apps' own gates need `CRAFT_FONTS_DIR` and `CRAFT_I18N_DIR` pointing at
real checkouts:
`/opt/hermes-agent/workspace/craft-i18n-mirror` and the fonts repo.

## Licensing and naming

All three are **Apache-2.0** (with a MIT file also present) and come from
`storytold/`. Rebrand to Workspace-family names, keeping upstream attribution
in `NOTICE` / `ATTRIBUTION.md` / licence files:

| Upstream | Suite name | app id | crate prefix |
|---|---|---|---|
| photocraft | **Photos** | `ph.photos` | `iridium-photos-*` |
| filmcraft | **Film** | `ph.film` | `iridium-film-*` |
| lightcraft | **Light** | `ph.light` | `iridium-light-*` |

**Persistent, protocol, cryptographic and wire-format identifiers are NOT
branding.** Write only the new name where safe, but READ both old and new:
profile folder names, file format magic, database schema, cache keys, keyring
service names, protocol constants, MIME types, and anything hashed into a path.
A blanket sweep that breaks a persistent identifier loses users' data.

## What "done" means for an app

1. The app's own repository builds and its own gate passes, **after** the rename.
2. A shell crate exists with the exact host API above.
3. The app's own binary still works standalone (the shell crate must not break it).
4. The suite compiles with the app linked, and the app draws in the suite.
5. The file types the app opens are routed to it, and the same list is in the
   routing table, the desktop/MIME metadata, and the Windows installer's offers.
6. The repository is cloned in **both** CI and release workflows (two lists).
7. The app's own repo has the new shell crate in its layer table, and the app's
   dependency on its shell has a same-layer exception.

## Verify before you claim

- A printed diagnostic in a test is NOT a contract. Read the assertion.
- A SKIPPED gate step is not a passing gate step.
- macOS- and Windows-only code paths cannot be compiled on this Linux box. Say
  which paths went unverified, and then close the gap with
  `cargo check -p <shell> --all-targets --target x86_64-pc-windows-msvc`.
