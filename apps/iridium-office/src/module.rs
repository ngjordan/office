//! The one trait the suite needs from a hosted app.
//!
//! Defined here and nowhere else. Each app's own crate exposes its own `Shell` with a
//! slightly different shape; the adapters in [`crate::modules`] bridge them to this.

use std::path::PathBuf;

/// Which app a module is. The suite's identity is the set of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Sheets,
    Docs,
    Cad,
    Decks,
    Pdf,
    Mail,
}

impl Kind {
    /// Every kind, in the order the rail shows them.
    pub const ALL: [Kind; 6] = [
        Kind::Sheets,
        Kind::Docs,
        Kind::Cad,
        Kind::Decks,
        Kind::Pdf,
        Kind::Mail,
    ];

    /// The product name, as the apps brand themselves.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Sheets => "Sheets",
            Kind::Docs => "Docs",
            Kind::Cad => "Cad",
            Kind::Decks => "Decks",
            Kind::Pdf => "PDF",
            Kind::Mail => "Mail",
        }
    }

    /// The two-letter mark that identifies this app.
    ///
    /// Two letters, not an icon: the suite ships no per-app artwork, and a mark
    /// that needs no asset still tells the apps apart at a glance.
    pub fn mark(self) -> &'static str {
        match self {
            Kind::Sheets => "Sh",
            Kind::Cad => "Ca",
            Kind::Docs => "Do",
            Kind::Decks => "De",
            Kind::Pdf => "PF",
            Kind::Mail => "Ma",
        }
    }

    /// This app's accent colour, in the light theme.
    ///
    /// Each app keeps its own identity colour. It tints the rail tile and the
    /// document tab, so a user can tell which app owns a document without
    /// reading the name.
    pub fn accent(self) -> egui::Color32 {
        match self {
            Kind::Sheets => egui::Color32::from_rgb(0x0B, 0x6E, 0x4F),
            Kind::Cad => egui::Color32::from_rgb(0x6D, 0x4B, 0xC3),
            Kind::Docs => egui::Color32::from_rgb(0x24, 0x5D, 0xB5),
            Kind::Decks => egui::Color32::from_rgb(0xA3, 0x4E, 0x16),
            Kind::Pdf => egui::Color32::from_rgb(0xB6, 0x37, 0x43),
            Kind::Mail => egui::Color32::from_rgb(0x92, 0x60, 0x1B),
        }
    }

    /// This app's accent colour, in the dark theme.
    pub fn accent_dark(self) -> egui::Color32 {
        match self {
            Kind::Sheets => egui::Color32::from_rgb(0x70, 0xC7, 0xA3),
            Kind::Cad => egui::Color32::from_rgb(0xBC, 0xA2, 0xF2),
            Kind::Docs => egui::Color32::from_rgb(0x91, 0xB8, 0xFF),
            Kind::Decks => egui::Color32::from_rgb(0xEF, 0xA4, 0x6E),
            Kind::Pdf => egui::Color32::from_rgb(0xF1, 0x8C, 0x96),
            Kind::Mail => egui::Color32::from_rgb(0xDD, 0xB6, 0x6E),
        }
    }

    /// A short description for the home screen.
    pub fn blurb(self) -> &'static str {
        match self {
            Kind::Sheets => "Spreadsheets",
            Kind::Docs => "Documents",
            Kind::Cad => "Drawings",
            Kind::Decks => "Presentations",
            Kind::Pdf => "PDF",
            Kind::Mail => "Mail",
        }
    }
}

/// A hosted app.
///
/// The suite calls `logic` for every STARTED module (they have background work: autosave,
/// print, the control server) and `ui` only for the ACTIVE one.
pub trait Module {
    /// Which app this is.
    fn kind(&self) -> Kind;

    /// Background work. Runs every frame for every started module.
    fn logic(&mut self, ctx: &egui::Context);

    /// Raw input, for apps that need to see events before egui does.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput);

    /// Draw. Runs only for the active module.
    fn ui(&mut self, ui: &mut egui::Ui);

    /// Open a file.
    fn open_path(&mut self, path: &str);

    /// Persist the module's state.
    fn save_state(&mut self);

    /// Whether the module is safe to close.
    fn can_close(&self) -> bool;

    /// Persist what the module keeps in eframe's storage.
    ///
    /// eframe calls this on its autosave timer and at exit. It is NOT the exit path:
    /// `save_state` is. Calling `save_state` here made the suite quit by itself, because
    /// eframe autosaves every 30 seconds and Mail's `save_state` runs `file.exit`.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}

    /// Called after the frame, for apps whose dialogs or quit handling need the frame.
    ///
    /// True means the app wants the suite to close.
    fn after_pass(&mut self, _frame: &eframe::Frame) -> bool {
        false
    }

    /// The zoom this module uses.
    ///
    /// egui applies a zoom on the NEXT pass, so the context still holds the previous
    /// app's zoom when a module is built. The module states its own.
    fn zoom(&self) -> f32 {
        1.0
    }

    /// The font set this module installed.
    ///
    /// The suite reads the style and the zoom from the context itself (it captures the
    /// look live, after the module's logic ran). The fonts are the one thing it cannot
    /// read back before egui's first pass, so the module reports them.
    ///
    /// This is the set the module BUILDS. The suite keeps it up to date from the context
    /// as well, so faces the module adds while running (a document's own fonts) are kept.
    fn fonts(&self) -> egui::FontDefinitions;

    /// Re-install this module's native menu (macOS). A no-op where there is none.
    fn install_native_menu(&mut self);
}

/// How a module was started.
pub struct LaunchSpec {
    /// Files to open, already routed to this app.
    pub files: Vec<String>,
}

/// The suite's own launch options, parsed from the command line.
#[derive(Debug, Default)]
pub struct Options {
    /// Files named on the command line, routed by extension.
    pub files: Vec<PathBuf>,
}
