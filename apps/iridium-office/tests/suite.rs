//! Phase 2b: the suite switches between the five apps in one window, and the
//! active module's look is restored on every switch.
//!
//! This is the acceptance test for the suite itself. The Phase 0 test
//! (`five_in_one`) stays the regression gate that the five apps can share a process.

use iridium_office::suite::Suite;

/// Build a suite with no files pending.
fn suite() -> Suite {
    Suite::new(Vec::new())
}

/// A headless frame: run the suite's logic and UI once.
fn frame(suite: &mut Suite, ctx: &egui::Context, w: f32, h: f32) -> usize {
    let mut raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(w, h),
        )),
        ..Default::default()
    };
    suite.raw_input_hook(ctx, &mut raw);
    let mut out = ctx.run_ui(raw, |root| {
        let ctx = root.ctx().clone();
        suite.logic(&ctx);
        egui::CentralPanel::default().show(root, |ui| {
            suite.ui(ui, None);
        });
    });
    let n = out.shapes.len();
    out.textures_delta.clear(); // headless: no renderer to apply them
    n
}

/// The suite starts empty and shows the home screen.
#[test]
fn starts_on_the_home_screen() {
    let ctx = egui::Context::default();
    let mut s = suite();
    assert!(
        s.active().is_none(),
        "a new suite must not have an active module"
    );
    assert!(
        frame(&mut s, &ctx, 1600.0, 1000.0) > 0,
        "the home screen painted nothing"
    );
    assert_eq!(
        s.started_len(),
        0,
        "no module should start until it is opened"
    );
}

/// Opening an app starts it, and switching back and forth keeps working.
#[test]
fn opens_an_app_and_switches_between_them() {
    let ctx = egui::Context::default();
    let mut s = suite();

    for kind in iridium_office::module::Kind::ALL {
        s.activate(kind, &ctx, None);
        assert_eq!(s.active(), Some(kind));
        assert!(
            frame(&mut s, &ctx, 1600.0, 1000.0) > 0,
            "{kind:?} painted nothing"
        );
    }
    // All five are started now, and the switch back to the first still works.
    assert_eq!(s.started_len(), 5, "all five modules should be started");
    let first = iridium_office::module::Kind::ALL[0];
    s.activate(first, &ctx, None);
    assert_eq!(s.active(), Some(first));
    assert!(
        frame(&mut s, &ctx, 1600.0, 1000.0) > 0,
        "{first:?} painted nothing on the way back"
    );
}

/// The active module's look is restored after a switch: the suite re-issues the
/// module's own fonts, style and zoom, so the previous module's look does not
/// show through.
#[test]
fn the_active_modules_look_is_restored_on_a_switch() {
    let ctx = egui::Context::default();
    let mut s = suite();

    let sheets = iridium_office::module::Kind::Sheets;
    let docs = iridium_office::module::Kind::Docs;

    s.activate(sheets, &ctx, None);
    frame(&mut s, &ctx, 1600.0, 1000.0);
    let sheets_style = ctx.global_style().as_ref().clone();
    let sheets_zoom = ctx.zoom_factor();

    // Switch to the other app, then back.
    s.activate(docs, &ctx, None);
    frame(&mut s, &ctx, 1600.0, 1000.0);
    s.activate(sheets, &ctx, None);
    frame(&mut s, &ctx, 1600.0, 1000.0);

    assert_eq!(
        ctx.zoom_factor(),
        sheets_zoom,
        "the zoom factor was not restored on the switch back"
    );
    // The style must be Sheets' own, not Docs' and not egui's default.
    assert_eq!(
        ctx.global_style().as_ref().clone(),
        sheets_style,
        "the style was not restored on the switch back"
    );
}

/// A runtime change to the active module's own look SURVIVES.
///
/// The suite restores the active module's look before it draws, so it must restore the
/// look the module currently wants — not a snapshot from when it was built. A snapshot
/// would revert a theme toggle or an interface-size change made while the module was
/// active.
#[test]
fn a_runtime_look_change_of_the_active_module_survives() {
    let ctx = egui::Context::default();
    let mut s = suite();

    let sheets = iridium_office::module::Kind::Sheets;
    s.activate(sheets, &ctx, None);
    frame(&mut s, &ctx, 1600.0, 1000.0);

    // The app changes its own zoom while it is active (the user picked a different
    // interface size). The suite must not undo it.
    ctx.set_zoom_factor(1.75);
    frame(&mut s, &ctx, 1600.0, 1000.0);
    assert_eq!(
        ctx.zoom_factor(),
        1.75,
        "the suite reverted the active module's own zoom change"
    );
}

/// A file is routed to the app that owns it.
#[test]
fn a_file_opens_in_the_app_that_owns_it() {
    let ctx = egui::Context::default();
    let mut s = Suite::new(vec![(
        iridium_office::module::Kind::Sheets,
        vec!["/tmp/does-not-exist.xlsx".to_string()],
    )]);
    assert_eq!(
        s.first_pending(),
        Some(iridium_office::module::Kind::Sheets),
        "an .xlsx belongs to Sheets"
    );
    s.activate(iridium_office::module::Kind::Sheets, &ctx, None);
    assert_eq!(s.active(), Some(iridium_office::module::Kind::Sheets));
    assert!(frame(&mut s, &ctx, 1600.0, 1000.0) > 0);
}
