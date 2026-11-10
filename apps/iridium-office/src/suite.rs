//! The suite: one window, a left rail, one app shown at a time.
//!
//! Two rules drive the whole design:
//!
//! 1. **Only the active module draws.** `logic` runs for every STARTED module (they have
//!    background work: autosave, print, the control server); `ui` runs only for the active
//!    one. Each module draws inside `ui.push_id(kind, ..)` so widget memory does not leak
//!    between modules.
//! 2. **The active module's look is restored on every frame.** Every app replaces the
//!    shared context's fonts, style and zoom, so without this the previous module's look
//!    shows through. The module's own `fonts()`/`style()`/`zoom()` are re-issued after the
//!    background modules have run.

use crate::module::{Kind, Module};
use crate::modules;

/// The suite's state.
pub struct Suite {
    /// The started modules, by kind.
    started: Vec<(Kind, Box<dyn Module>)>,
    /// Which module is showing.
    active: Option<Kind>,
    /// Files to open when a module is first started.
    pending: Vec<(Kind, Vec<String>)>,
}

impl Suite {
    /// A suite with no modules started yet.
    pub fn new(files: Vec<(Kind, Vec<String>)>) -> Self {
        Self {
            started: Vec::new(),
            active: None,
            pending: files,
        }
    }

    /// The module currently showing.
    pub fn active(&self) -> Option<Kind> {
        self.active
    }

    /// The first app that has files waiting to be opened.
    pub fn first_pending(&self) -> Option<Kind> {
        self.pending.first().map(|(k, _)| *k)
    }

    /// The number of started modules.
    pub fn started_len(&self) -> usize {
        self.started.len()
    }

    /// Start a module if it is not started, and make it active.
    pub fn activate(
        &mut self,
        kind: Kind,
        ctx: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
    ) {
        if !self.started.iter().any(|(k, _)| *k == kind) {
            let files = self
                .pending
                .iter()
                .find(|(k, _)| *k == kind)
                .map(|(_, f)| f.clone())
                .unwrap_or_default();
            let module: Box<dyn Module> = match kind {
                Kind::Sheets => Box::new(modules::sheets::Adapter::new(ctx, storage, files)),
                Kind::Docs => Box::new(modules::docs::Adapter::new(ctx, storage, files)),
                Kind::Cad => Box::new(modules::cad::Adapter::new(ctx, storage, files)),
                Kind::Decks => Box::new(modules::decks::Adapter::new(ctx, storage, files)),
                Kind::Pdf => Box::new(modules::pdf::Adapter::new(ctx, storage, files)),
            };
            self.started.push((kind, module));
        }
        self.active = Some(kind);
        self.restore_look(kind, ctx);
    }

    /// Re-issue the active module's fonts, style and zoom into the shared context.
    fn restore_look(&mut self, kind: Kind, ctx: &egui::Context) {
        let Some((_, m)) = self.started.iter_mut().find(|(k, _)| *k == kind) else {
            return;
        };
        ctx.set_fonts(m.fonts());
        ctx.set_global_style(m.style());
        ctx.set_zoom_factor(m.zoom());
    }

    /// Background work for every started module.
    pub fn logic(&mut self, ctx: &egui::Context) {
        for (_, m) in &mut self.started {
            m.logic(ctx);
        }
        // A background module may have replaced the shared look. Put the active
        // module's look back, so the frame is drawn in the active module's style.
        if let Some(k) = self.active {
            self.restore_look(k, ctx);
        }
    }

    /// Raw input for every started module.
    pub fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        for (_, m) in &mut self.started {
            m.raw_input_hook(ctx, raw);
        }
    }

    /// The rail, and the active module's UI.
    pub fn ui(&mut self, ui: &mut egui::Ui, storage: Option<&dyn eframe::Storage>) {
        let ctx = ui.ctx().clone();
        let Some(active) = self.active else {
            self.home(ui, storage, &ctx);
            return;
        };

        // The rail. The click is collected and acted on after the panel closes, so
        // the borrow of `self` inside the panel does not overlap the switch.
        let mut chosen = None;
        egui::Panel::left("rail")
            .default_size(84.0)
            .resizable(false)
            .show(ui, |ui| {
                for kind in Kind::ALL {
                    let started = self.started.iter().any(|(k, _)| *k == kind);
                    let selected = active == kind;
                    let mut label = egui::RichText::new(kind.label());
                    if selected {
                        label = label.strong();
                    } else if !started {
                        label = label.weak();
                    }
                    if ui.add(egui::Button::new(label)).clicked() {
                        chosen = Some(kind);
                    }
                    ui.add_space(4.0);
                }
            });
        if let Some(kind) = chosen {
            self.activate(kind, &ctx, storage);
            return;
        }

        // The active module draws, in its own id space.
        let Some((_, m)) = self.started.iter_mut().find(|(k, _)| *k == active) else {
            return;
        };
        ui.push_id(active, |ui| m.ui(ui));
    }

    /// The home screen: one button per app.
    fn home(&mut self, ui: &mut egui::Ui, storage: Option<&dyn eframe::Storage>, ctx: &egui::Context) {
        let mut chosen = None;
        egui::CentralPanel::default().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.heading("Office");
                ui.add_space(20.0);
                for kind in Kind::ALL {
                    if ui.button(kind.label()).clicked() {
                        chosen = Some(kind);
                    }
                    ui.add_space(4.0);
                }
            });
        });
        if let Some(kind) = chosen {
            self.activate(kind, ctx, storage);
        }
    }

    /// Whether every started module is safe to close.
    pub fn can_close(&self) -> bool {
        self.started.iter().all(|(_, m)| m.can_close())
    }

    /// Persist every started module's state.
    pub fn save_state(&mut self) {
        for (_, m) in &mut self.started {
            m.save_state();
        }
    }
}
