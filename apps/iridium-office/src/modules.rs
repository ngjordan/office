//! The five adapters, one per app.
//!
//! Each owns its app's `Shell` and forwards the trait's methods to it. The only work
//! here is reconciling the small differences between the apps:
//!
//! - `install_native_menu` takes `&self` in Sheets, Docs, Cad and Decks, but `&mut self`
//!   in PDF (it stores the menu inside the app). The trait uses `&mut self` for all of them,
//!   and the four that need `&self` just ignore the extra access.
//! - `Launch` differs per app, so each adapter builds its own.

use crate::module::{Kind, Module};

macro_rules! adapter {
    ($mod_name:ident, $kind:expr, $shell:ty, $launch:ty, $doc:expr) => {
        pub mod $mod_name {
            use super::*;

            /// The hosted app.
            pub struct Adapter {
                shell: $shell,
            }

            impl Adapter {
                /// Build the module. `files` are this app's own files, already routed.
                pub fn new(
                    ctx: &egui::Context,
                    storage: Option<&dyn eframe::Storage>,
                    files: Vec<String>,
                ) -> Self {
                    let launch: $launch = $doc(files);
                    Self {
                        shell: <$shell>::new_for_host(ctx, storage, launch),
                    }
                }
            }

            impl Module for Adapter {
                fn kind(&self) -> Kind {
                    $kind
                }
                fn logic(&mut self, ctx: &egui::Context) {
                    self.shell.logic(ctx);
                }
                fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
                    self.shell.raw_input_hook(ctx, raw);
                }
                fn ui(&mut self, ui: &mut egui::Ui) {
                    self.shell.ui(ui);
                }
                fn open_path(&mut self, path: &str) {
                    self.shell.open_path(path);
                }
                fn save_state(&mut self) {
                    self.shell.save_state();
                }
                fn can_close(&self) -> bool {
                    self.shell.can_close()
                }
                fn fonts(&self) -> egui::FontDefinitions {
                    self.shell.fonts()
                }
                fn install_native_menu(&mut self) {
                    self.shell.install_native_menu();
                }
            }
        }
    };
}

adapter!(
    sheets,
    Kind::Sheets,
    iridium_sheets_shell::Shell,
    iridium_sheets_shell::Launch,
    |files| iridium_sheets_shell::Launch {
        files,
        sample: None,
        control_port: None,
        control_token: None,
        viewport: egui::ViewportBuilder::default(),
        prefs: true,
        config_dir: None,
        #[cfg(target_os = "macos")]
        inbox: fmv_macos_events::Inbox::default(),
    }
);

adapter!(
    docs,
    Kind::Docs,
    iridium_docs_shell::Shell,
    iridium_docs_shell::Launch,
    |files| iridium_docs_shell::Launch {
        files,
        other_files: Vec::new(),
        sample: false,
        control_port: None,
        window: None,
        window_builder: egui::ViewportBuilder::default(),
        #[cfg(target_os = "macos")]
        apple_events: fmv_macos_events::Inbox::default(),
    }
);

adapter!(
    cad,
    Kind::Cad,
    iridium_cad_shell::Shell,
    iridium_cad_shell::Launch,
    |files: Vec<String>| iridium_cad_shell::Launch {
        files: files.into_iter().map(std::path::PathBuf::from).collect(),
        ..Default::default()
    }
);

adapter!(
    decks,
    Kind::Decks,
    iridium_decks_shell::Shell,
    iridium_decks_shell::Launch,
    |files| iridium_decks_shell::Launch {
        files,
        sample: false,
        show: false,
        control_port: None,
        #[cfg(target_os = "macos")]
        apple_events: fmv_macos_events::Inbox::default(),
    }
);

adapter!(
    pdf,
    Kind::Pdf,
    iridium_pdf_shell::Shell,
    iridium_pdf_shell::Launch,
    |files| iridium_pdf_shell::Launch {
        files,
        ..Default::default()
    }
);
