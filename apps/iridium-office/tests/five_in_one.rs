//! Phase 0: the apps' UI crates run in one process on one egui::Context.
use eframe::App as _;

#[test]
fn five_in_one() {
    let ctx = egui::Context::default();
    let mut sheets = iridium_ui_egui::SheetApp::new(
        iridium_engine::Session::new(),
        iridium_ui_egui::Services::default(),
    );
    let mut docs = iridium_docs_ui_egui::WordApp::new(
        iridium_docs_engine::Session::new(iridium_docs_engine::sample::sample_document()),
        iridium_docs_ui_egui::Services::default(),
    );
    let mut cad = iridium_cad_ui_egui::CadApp::new(
        iridium_cad_engine::Session::empty(),
        iridium_cad_ui_egui::Services::default(),
    );
    let mut decks = iridium_decks_ui_egui::SlideApp::new(
        iridium_decks_engine::Session::new(),
        iridium_decks_ui_egui::Services::default(),
    );
    let mut pdf = iridium_pdf_ui_egui::PdfApp::new();
    let mut mail = iridium_mail_ui_egui::App::new(iridium_mail_engine::sample::sample_session_at(
        iridium_mail_engine::model::DateTime::now_utc(),
    ));
    let mut frame = eframe::Frame::_new_kittest();

    let mut total_shapes = 0;
    for n in 0..3 {
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 1000.0),
            )),
            ..Default::default()
        };
        sheets.raw_input_hook(&ctx, &mut raw);
        docs.raw_input_hook(&ctx, &mut raw);
        cad.raw_input_hook(&ctx, &mut raw);
        decks.raw_input_hook(&ctx, &mut raw);
        pdf.raw_input_hook(&ctx, &mut raw);
        // Mail has no raw input hook of its own; the suite's adapter calls the same
        // nothing, so the gate calls it too.
        let _ = &mail;
        let mut out = ctx.run_ui(raw, |root| {
            let ctx = root.ctx().clone();
            let ctx = &ctx;
            sheets.logic(ctx);
            docs.logic(ctx);
            cad.logic(ctx);
            decks.logic(ctx);
            pdf.logic(ctx, &mut frame);
            mail.poll_sync(ctx);
            egui::CentralPanel::default().show(root, |ui| {
                let size = ui.available_size();
                macro_rules! region {
                    ($id:expr, $draw:expr) => {
                        ui.push_id($id, |ui| {
                            let r = egui::Rect::from_min_size(ui.cursor().min, size);
                            ui.scope_builder(egui::UiBuilder::new().max_rect(r), $draw);
                        });
                    };
                }
                region!("sheets", |ui: &mut egui::Ui| sheets.ui(ui));
                region!("docs", |ui: &mut egui::Ui| docs.ui(ui));
                region!("cad", |ui: &mut egui::Ui| cad.ui(ui));
                region!("decks", |ui: &mut egui::Ui| decks.ui(ui));
                region!("pdf", |ui: &mut egui::Ui| pdf.ui(ui, &mut frame));
                region!("mail", |ui: &mut egui::Ui| mail.ui_root(ui));
            });
        });
        total_shapes += out.shapes.len();
        out.textures_delta.clear(); // headless: no renderer to apply them
        eprintln!("frame {n}: {} shapes", out.shapes.len());
    }
    assert!(total_shapes > 0, "no shapes painted");
}
