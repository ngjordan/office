# Office suite

One window, one egui context, five apps: **Sheets**, **Docs**, **Cad**, **Decks**, **PDF**.

This repository holds the Office host only. The five apps stay in their own repositories
and are pulled in as path dependencies, so the standalone apps keep shipping on their own.

| App | Repository | Branch | Path (relative to this repo) |
|---|---|---|---|
| Sheets | `ngjordan/sheets` | `feature/sheets-shell-library` | `../gc-gridcraft` |
| Docs | `ngjordan/docs` | `feature/docs-shell-library` | `../wc-wordcraft` |
| Cad | `ngjordan/cad` | `feature/cad-shell-library` | `../cc-cadcraft` |
| Decks | `ngjordan/decks` | `feature/decks-shell-library` | `../dc-deckcraft` |
| PDF | `ngjordan/pdf` | `feature/pdf-shell-library` | `../pdfcraft` |

Cloning this repo alone does **not** build: clone the five app repositories as siblings
of this directory, on the branches above.

## Build and test

```sh
cd office
export CARGO_BUILD_JOBS=4                      # this box OOMs on unbounded parallelism
export CRAFT_FONTS_DIR=/tmp/craft-fonts
export CRAFT_I18N_DIR=/tmp/craft-i18n
export PKG_CONFIG_PATH=/opt/hermes-agent/workspace/alsa-stub/pkgconfig
export LIBRARY_PATH=/opt/hermes-agent/workspace/alsa-stub/lib
umask 022
cargo test -p iridium-office -j 1
```

`-j 1` matters: the test binaries are ~2.8 GB with debug info, and `cargo test` links up
to `CARGO_BUILD_JOBS` of them at once — concurrent links get the build OOM-killed
(exit -9, no error message).

## Tests

| Test | What it holds down |
|---|---|
| `tests/five_in_one.rs` | Phase 0 regression gate: all five apps construct, paint and share one egui context |
| `tests/suite.rs` | Phase 2b: the switcher, file routing, and the look rules across a switch |
| `src/suite.rs` unit tests | the per-module look (style, zoom, fonts), using a fake module that owns its look |

## How the look works

The five apps share ONE egui context, and each replaces that context's fonts, style and
zoom with its own. Each app decides whether to re-install its look against **its own**
memory, not against the context — so an app that has already installed its look will not
install it again, and would otherwise draw in whatever look the previous app left behind.

The suite therefore keeps a **live** look per module:

1. background modules run their logic first, the active module last;
2. each module's look is restored **before** its own logic and captured **after** it;
3. the active module's look is restored again before it draws.

A construction-time snapshot is deliberately avoided: it would revert the user's runtime
theme or interface-size change and would drop faces an app added while running (PDF's
document fonts).

egui 0.37 details that shape this: styles and theme apply immediately, while
`set_fonts` and `set_zoom_factor` land a pass later and their pending values are private —
so only applied values can be read back. The zoom is written straight into
`Options::zoom_factor`, because egui's `set_zoom_factor` requests a repaint on every call
and the suite re-issues it every frame.
