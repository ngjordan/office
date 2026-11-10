//! The suite binary: one window, five apps.
//!
//! eframe 0.37's `App` trait splits the frame into `logic(ctx, frame)` and `ui(ui, frame)`,
//! so the suite's own frame order is:
//!
//! 1. every started module's `logic` (background work),
//! 2. re-issue the active module's look (a background module may have replaced it),
//! 3. the rail and the active module's `ui`.

use iridium_office::{route, suite::Suite};

/// The suite's own state, owned by eframe.
struct App {
    suite: Suite,
}

impl eframe::App for App {
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.suite.save_state();
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.suite.logic(ctx);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.suite.raw_input_hook(ctx, raw);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // `Frame::storage()` is public, so a module started later still gets the
        // frame's storage -- the reason `CreationContext` could not be stored.
        self.suite.ui(ui, frame.storage());
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.suite.save_state();
    }
}

fn main() -> eframe::Result<()> {
    let files: Vec<std::path::PathBuf> = std::env::args_os().skip(1).map(Into::into).collect();
    let routed = route::route(&files);

    let viewport = egui::ViewportBuilder::default()
        .with_title("Office")
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([720.0, 520.0]);

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Office",
        options,
        Box::new(|cc| {
            let storage = cc.storage;
            let mut suite = Suite::new(routed);
            // Open the app that owns the first file, so a launch with a file shows it.
            if let Some(kind) = suite.first_pending() {
                suite.activate(kind, &cc.egui_ctx, storage);
            }
            Ok(Box::new(App { suite }))
        }),
    )
}
