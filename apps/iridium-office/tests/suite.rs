//! Phase 2b: the suite switches between the apps in one window, and the
//! active module's look is restored on every switch.
//!
//! This is the acceptance test for the suite itself. The Phase 0 test
//! (`five_in_one`) stays the regression gate that the apps can share a process.

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
    // Every kind is started now, and the switch back to the first still works. The count
    // comes from the kinds themselves: it did not change when a sixth app was added.
    let all = iridium_office::module::Kind::ALL;
    assert_eq!(s.started_len(), all.len(), "every module should be started");
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

/// The tab strip: the suite knows which documents are open.
#[test]
fn the_suite_tracks_its_open_documents() {
    let ctx = egui::Context::default();
    let sheets = iridium_office::module::Kind::Sheets;
    let docs = iridium_office::module::Kind::Docs;
    let mut s = Suite::new(vec![(sheets, vec!["/tmp/book.xlsx".to_string()])]);

    // The file given at start-up is an open document.
    assert_eq!(s.open_documents().len(), 1);
    assert_eq!(s.open_documents()[0].1, "/tmp/book.xlsx");

    // Opening one more adds a tab, and opening the same one again does not.
    s.open_document(docs, "/tmp/letter.docx", &ctx, None);
    assert_eq!(s.open_documents().len(), 2);
    s.open_document(docs, "/tmp/letter.docx", &ctx, None);
    assert_eq!(s.open_documents().len(), 2, "the same file is one tab");

    // The tab strip draws.
    assert!(frame(&mut s, &ctx, 1600.0, 1000.0) > 0);
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

/// The Windows installer must offer exactly the extensions the suite routes.
/// A file type that routes but is not offered opens in nothing; one that is
/// offered but does not route opens in the wrong module.
#[test]
fn the_windows_installer_offers_exactly_the_routed_extensions() {
    let wxs = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packaging/windows/iridium-office.wxs"
    ))
    .expect("the Windows installer source must exist");

    // The suite's own format is registered as an Extension, so it is the default
    // handler for it. Every other type is only offered: an Extension for a foreign
    // type would take it over from whatever the user already had.
    let owned: std::collections::HashSet<String> = wxs
        .lines()
        .filter_map(|l| l.trim().strip_prefix("<Extension Id=\""))
        .filter_map(|l| l.split('"').next())
        .map(|s| s.to_string())
        .collect();
    let offered: std::collections::HashSet<String> = wxs
        .lines()
        .filter(|l| l.contains("OpenWithProgids"))
        .filter_map(|l| {
            // the row is Key=".<ext>\OpenWithProgids"
            let rest = &l[l.find("Key=\".")? + 6..];
            let end = rest.find('\\')?;
            Some(rest[..end].to_string())
        })
        .collect();

    let routed: std::collections::HashSet<String> = iridium_office::route::table()
        .into_iter()
        .map(|(e, _)| e.to_string())
        .collect();

    let all: std::collections::HashSet<String> = owned.union(&offered).cloned().collect();
    let missing: Vec<_> = routed.difference(&all).collect();
    let extra: Vec<_> = all.difference(&routed).collect();
    assert!(
        missing.is_empty(),
        "routed but not offered on Windows: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "offered on Windows but not routed: {extra:?}"
    );

    assert_eq!(
        owned.iter().cloned().collect::<Vec<_>>(),
        vec!["iridium-decks".to_string()],
        "only the suite's own format may be registered as an Extension"
    );
}

/// Every offered extension needs an open command, or double-clicking the file
/// does nothing.
#[test]
fn every_offered_extension_has_an_open_command() {
    let wxs = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packaging/windows/iridium-office.wxs"
    ))
    .expect("the Windows installer source must exist");
    let offered = wxs.matches("\\OpenWithProgids\"").count();
    assert!(offered > 0);
    // Every ProgId key carries exactly one open command.
    let progid_blocks: Vec<&str> = wxs
        .split("<RegistryKey Root=\"HKCR\" Key=\"Office.")
        .skip(1)
        .collect();
    assert!(!progid_blocks.is_empty());
    for block in progid_blocks {
        let key_end = block.find("</RegistryKey>").unwrap_or(block.len());
        let body = &block[..key_end];
        assert_eq!(
            body.matches("shell\\open\\command").count(),
            1,
            "each ProgId needs exactly one open command"
        );
    }
}

#[test]
fn every_progid_the_policy_names_is_one_the_installer_defines() {
    // The policy is what the release pipeline checks the installer against. A ProgId
    // spelled differently in the two files is a mismatch the check throws on.
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/windows");
    let policy =
        std::fs::read_to_string(base.join("install-policy.json")).expect("install-policy.json");
    let wxs = std::fs::read_to_string(base.join("iridium-office.wxs")).expect("iridium-office.wxs");
    assert!(
        !policy.contains("Office.Office."),
        "a ProgId is written with the prefix twice"
    );
    for name in [
        "Message",
        "Mailbox",
        "Calendar",
        "Contact",
        "OutlookMessage",
        "OutlookDataFile",
    ] {
        let progid = format!("Office.{name}");
        assert!(
            policy.contains(&format!("\"{progid}\"")),
            "the policy names {progid}"
        );
        assert!(
            wxs.contains(&format!("Key=\"{progid}\"")),
            "the installer defines {progid}"
        );
    }
}
