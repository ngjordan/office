//! Headless screenshots of the suite's chrome.
//!
//! Renders the real rail, tab strip and home screen offscreen (wgpu), so the
//! result is what the app draws, not a mockup.
//!
//! `cargo run -p iridium-office --example chrome_shot -- <out-dir>`
//!
//! Writes one PNG per view: the home screen, and the suite with documents
//! open, in the light and dark themes.

use iridium_office::suite::Suite;

/// Where the pictures go.
fn out_dir() -> std::path::PathBuf {
    std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/workspace-shots"))
}

/// A suite with a few documents open, so the tab strip has something to show.
fn suite_with_documents() -> Suite {
    let mut s = Suite::new(Vec::new());
    for (kind, name) in [
        (iridium_office::module::Kind::Sheets, "Q3 forecast.xlsx"),
        (iridium_office::module::Kind::Docs, "Service agreement.docx"),
        (iridium_office::module::Kind::Cad, "Site plan.dwg"),
        (iridium_office::module::Kind::Pdf, "Annual report.pdf"),
        (iridium_office::module::Kind::Mail, "Inbox"),
    ] {
        s.open_for_test(kind, name);
    }
    // The tab strip only shows once an app is showing.
    s.show_for_test(iridium_office::module::Kind::Docs);
    s
}

/// Render one view and save it.
fn shot(name: &str, dark: bool, suite: Suite, w: f32, h: f32, dir: &std::path::Path) {
    let theme = if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(w, h))
        .with_pixels_per_point(2.0)
        .with_theme(theme)
        .with_max_steps(100_000)
        .wgpu()
        .build_ui_state(
            |ui, suite: &mut Suite| {
                let ctx = ui.ctx().clone();
                suite.logic(&ctx);
                egui::CentralPanel::default().show(ui, |ui| {
                    suite.ui(ui, None);
                });
            },
            suite,
        );
    harness.input_mut().max_texture_side = Some(8192);
    // A few frames: the first installs the chrome font, the rest settle.
    for _ in 0..6 {
        harness.step();
    }
    let path = dir.join(name);
    match harness.render() {
        Ok(img) => {
            img.save(&path).expect("save png");
            println!("{}", path.display());
        }
        Err(e) => eprintln!("render failed for {name}: {e}"),
    }
}

/// Render one real browser page through the frame pipeline, and save the pixels.
///
/// This is the whole path: a real Chromium paints a page, the screencast
/// carries it, the worker decodes it, and the result is the picture.
fn shot_browser(dir: &std::path::Path) {
    use iridium_office::browser::Engine;

    if iridium_office::browser::launch::find_browser().is_none() {
        eprintln!("no Chromium-family browser installed; skipping the browser shot");
        return;
    }
    let profile =
        iridium_office::browser::launch::private_profile_dir("chrome-shot").expect("profile");
    let mut engine = match Engine::start_in(Some(profile)) {
        Ok(engine) => engine,
        Err(e) => {
            eprintln!("the browser did not start: {e}");
            return;
        }
    };
    engine
        .call(
            headless_chrome::protocol::cdp::Page::Navigate {
                url: "data:text/html,<body style='margin:0;font:600 42px system-ui;background:%230B6E4F;color:white;padding:40px'>Workspace</body>".to_string(),
                referrer: None,
                transition_Type: None,
                frame_id: None,
                referrer_policy: None,
            },
        )
        .expect("navigate");
    std::thread::sleep(std::time::Duration::from_secs(2));
    engine
        .start_screencast(iridium_office::browser::frame::FrameFormat::Jpeg, 900, 500)
        .expect("the screencast starts");

    let mut frame = None;
    for _ in 0..60 {
        if let Some(f) = engine.take_frame() {
            frame = Some(f);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let Some(frame) = frame else {
        eprintln!("no frame arrived");
        return;
    };
    // The frame's pixels become a PNG, exactly as the suite would draw them.
    // The pixels are RGBA, four bytes each.
    let image = image::RgbaImage::from_raw(frame.width, frame.height, frame.pixels.clone())
        .map(image::DynamicImage::ImageRgba8)
        .expect("the frame's size matches its pixels");
    let path = dir.join("browser-frame.png");
    image.save(&path).expect("save png");
    println!("{}", path.display());
}

fn main() {
    let dir = out_dir();
    std::fs::create_dir_all(&dir).expect("output directory");
    let (w, h) = (1440.0, 900.0);

    shot_browser(&dir);

    for (dark, suffix) in [(false, "light"), (true, "dark")] {
        shot(
            &format!("home-{suffix}.png"),
            dark,
            Suite::new(Vec::new()),
            w,
            h,
            &dir,
        );
        shot(
            &format!("suite-{suffix}.png"),
            dark,
            suite_with_documents(),
            w,
            h,
            &dir,
        );
    }
}
