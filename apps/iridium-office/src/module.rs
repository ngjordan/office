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
