//! The suite: one window, a left rail, one app shown at a time.
//!
//! Four rules drive the whole design:
//!
//! 1. **Only the active module draws.** `logic` runs for every STARTED module (they have
//!    background work: autosave, print, the control server); `ui` runs only for the active
//!    one. Each module draws inside `ui.push_id(kind, ..)` so widget memory does not leak
//!    between modules.
//! 2. **Each module's look is restored before its own logic runs.** Every app replaces the
//!    shared context's fonts, style and zoom with its own, and each app decides whether to
//!    re-install its look against ITS OWN memory, not against the context. So an app that
//!    has already installed its look will not install it again, and would otherwise draw
//!    in whatever look the previous app left behind. The suite puts the look back first.
//! 3. **Each module's look is captured after its own logic ran.** The capture is live, not
//!    a construction-time snapshot: a snapshot would revert the app's own runtime changes
//!    (a theme the user picked, an interface-size change) and would wipe fonts the app
//!    added while running (PDF's `pdf-source-*` document faces, Sheets' workbook fonts).
//! 4. **The active module's look is restored before it draws**, so a background module
//!    cannot leave its look behind.
//!
//! The background modules run their logic FIRST, so the active module's look is the last
//! one in the context when the frame is drawn.

use crate::look::{FontSig, Look};
use crate::module::{Kind, Module};
use crate::modules;

/// The suite's state.
pub struct Suite {
    /// The started modules, by kind.
    started: Vec<(Kind, Box<dyn Module>)>,
    /// The look each started module left in the context, captured after its logic ran.
    looks: Vec<(Kind, Look)>,
    /// Which module is showing.
    active: Option<Kind>,
    /// Files to open when a module is first started.
    pending: Vec<(Kind, Vec<String>)>,
    /// The documents this suite has open, in the order they were opened. The tab strip
    /// shows one per document.
    open_documents: Vec<(Kind, String)>,
    /// Whether egui has drawn a pass yet. The context's fonts cannot be read before that.
    saw_ui: bool,
    /// Whether the suite held the window's close this frame because a module has unsaved
    /// work. That decision wins over an app asking to quit.
    close_held: bool,
    /// The module that drew last frame. The context holds ITS look, so what the context
    /// holds is attributed to it and not to the module that is active now.
    last_drawn: Option<Kind>,
}

impl Suite {
    /// A suite with no modules started yet.
    pub fn new(files: Vec<(Kind, Vec<String>)>) -> Self {
        let open_documents: Vec<(Kind, String)> = files
            .iter()
            .flat_map(|(k, paths)| paths.iter().map(move |p| (*k, p.clone())))
            .collect();
        Self {
            started: Vec::new(),
            looks: Vec::new(),
            active: None,
            pending: files,
            open_documents,
            saw_ui: false,
            // Whether the close was held this frame.
            close_held: false,
            // The module that drew last frame.
            last_drawn: None,
        }
    }

    /// The documents this suite has open.
    pub fn open_documents(&self) -> &[(Kind, String)] {
        &self.open_documents
    }

    /// Open a document in the app that owns it, and remember it for the tab strip.
    pub fn open_document(
        &mut self,
        kind: Kind,
        path: &str,
        ctx: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
    ) {
        self.activate(kind, ctx, storage);
        if !self
            .open_documents
            .iter()
            .any(|(k, p)| *k == kind && p == path)
        {
            self.open_documents.push((kind, path.to_string()));
        }
        if let Some((_, m)) = self.started.iter_mut().find(|(k, _)| *k == kind) {
            m.open_path(path);
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
                Kind::Mail => Box::new(modules::mail::Adapter::new(ctx, storage, files)),
            };
            self.activate_module(kind, module, ctx);
        }
        self.active = Some(kind);
    }

    /// Start a module that has already been built.
    fn activate_module(&mut self, kind: Kind, module: Box<dyn Module>, ctx: &egui::Context) {
        // The module installed its own look into the context as it was built: capture
        // it, so the suite has something to put back.
        let mut look = Look::capture(ctx, module.fonts());
        // The context still holds the app that was showing: the module states its own.
        look.zoom = module.zoom();
        self.started.push((kind, module));
        self.looks.push((kind, look));
    }

    /// Put one module's look back into the context.
    ///
    /// `fonts` is off for the background modules: restoring a different font set rebuilds
    /// egui's font atlas, which is far too expensive to do for every module every frame.
    /// A background module does not draw, so it does not need its fonts.
    fn restore_look(&self, ctx: &egui::Context, kind: Kind, fonts: bool) {
        let Some((_, look)) = self.looks.iter().find(|(k, _)| *k == kind) else {
            return;
        };
        look.restore_styles(ctx);
        look.set_zoom(ctx);
        if fonts {
            look.restore_fonts(ctx);
        }
    }

    /// Read back what one module left in the context.
    ///
    /// The styles apply at once, so they are the module's own right after its logic ran.
    /// The fonts land a pass later, so they are only taken when the set CHANGED across
    /// this module's own logic: that is what makes the change attributable to it. Another
    /// module's fonts are never adopted as this one's.
    fn capture_look(&mut self, ctx: &egui::Context, kind: Kind, fonts_before: Option<FontSig>) {
        let fonts = self.saw_ui.then(|| Look::fonts_now(ctx));
        if let Some((_, look)) = self.looks.iter_mut().find(|(k, _)| *k == kind) {
            look.capture_styles(ctx);
            if let (Some(fonts), Some(before)) = (fonts, fonts_before) {
                let sig = Look::sig_of(&fonts);
                if sig != before {
                    look.fonts = fonts;
                    look.font_sig = sig;
                }
            }
        }
    }

    /// Take over what the context is applying now.
    ///
    /// The applied zoom is the active module's: it is the last module to have asked for
    /// one. The applied fonts may also hold faces the active module added while it drew
    /// (PDF's document faces), which the next restore must not throw away.
    fn adopt(&mut self, ctx: &egui::Context) {
        // The context holds the look of the module that drew last frame. After a switch
        // that is the app the user just left, not the one now active: attributing its
        // zoom to the new app made the new app's zoom wrong for good.
        let Some(drawn) = self.last_drawn.or(self.active) else {
            return;
        };
        let zoom = ctx.zoom_factor();
        if let Some((_, look)) = self.looks.iter_mut().find(|(k, _)| *k == drawn) {
            look.zoom = zoom;
        }
        if !self.saw_ui {
            return;
        }
        let fonts = Look::fonts_now(ctx);
        let sig = Look::sig_of(&fonts);
        // Another module's set, not this module's additions.
        let owned_by_other = self
            .looks
            .iter()
            .any(|(k, l)| *k != drawn && l.font_sig == sig);
        if owned_by_other {
            return;
        }
        if let Some((_, look)) = self.looks.iter_mut().find(|(k, _)| *k == drawn)
            && sig != look.font_sig
            && look.covers(&sig)
        {
            look.fonts = fonts;
            look.font_sig = sig;
        }
    }

    /// Background work for every started module.
    pub fn logic(&mut self, ctx: &egui::Context) {
        // A module with unsaved work has to be asked before the window closes: bring it
        // up so the prompt is drawn, and hold the close.
        if ctx.input(|i| i.viewport().close_requested()) && !self.can_close() {
            if let Some(kind) = self
                .started
                .iter()
                .find(|(_, m)| !m.can_close())
                .map(|(k, _)| *k)
            {
                self.active = Some(kind);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_held = true;
        }

        // What the context holds now is the module that drew last frame.
        self.adopt(ctx);

        // The background modules first, the active one last: its look is then the one in
        // the context when the frame is drawn.
        let active = self.active;
        let mut order: Vec<usize> = (0..self.started.len())
            .filter(|i| Some(self.started[*i].0) != active)
            .collect();
        if let Some(i) = self.started.iter().position(|(k, _)| Some(*k) == active) {
            order.push(i);
        }
        for i in order {
            let kind = self.started[i].0;
            let is_active = Some(kind) == active;
            self.restore_look(ctx, kind, is_active);
            // Only the active module's fonts are put back, so only its own font changes
            // can be attributed to it.
            let before = if is_active && self.saw_ui {
                Some(Look::sig_now(ctx))
            } else {
                None
            };
            self.started[i].1.logic(ctx);
            self.capture_look(ctx, kind, before);
        }
    }

    /// Raw input for every started module.
    pub fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        // A dropped file belongs to the app that owns it. Handing every drop to every
        // app meant a .pdf ended up in a spreadsheet.
        let dropped = std::mem::take(&mut raw.dropped_files);
        for (kind, m) in &mut self.started {
            let mine: Vec<std::sync::Arc<dyn egui::DroppedFile + Send + Sync>> = dropped
                .iter()
                .filter(|f| crate::route::kind_for(f.path()) == Some(*kind))
                .cloned()
                .collect();
            let saved = std::mem::replace(&mut raw.dropped_files, mine);
            m.raw_input_hook(ctx, raw);
            raw.dropped_files = saved;
        }
        raw.dropped_files = dropped;
    }

    /// The rail, and the active module's UI.
    pub fn ui(&mut self, ui: &mut egui::Ui, storage: Option<&dyn eframe::Storage>) {
        self.saw_ui = true;
        let ctx = ui.ctx().clone();
        let Some(active) = self.active else {
            self.home(ui, storage, &ctx);
            return;
        };

        // The tab strip: one tab per open document. The click is collected and acted on
        // after the panel closes, so the borrow of `self` inside the panel does not
        // overlap the switch.
        let mut chosen_doc: Option<(Kind, String)> = None;
        let mut closed: Option<usize> = None;
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, (kind, path)) in self.open_documents.iter().enumerate() {
                    let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_string();
                    ui.horizontal(|ui| {
                        if ui
                            .button(format!("{} \u{2014} {name}", kind.label()))
                            .clicked()
                        {
                            chosen_doc = Some((*kind, path.clone()));
                        }
                        if ui.small_button("\u{2715}").clicked() {
                            closed = Some(i);
                        }
                    });
                }
            });
        });
        if let Some((kind, _path)) = chosen_doc {
            // The document is already open: clicking its tab only brings its app up.
            // Opening it again imported it a second time.
            self.activate(kind, &ctx, storage);
        }
        if let Some(i) = closed {
            self.open_documents.remove(i);
        }

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

        // A background module may have left its look behind: put the active module's own
        // look back before it draws.
        self.restore_look(&ctx, active, true);

        // The active module draws, in its own id space.
        let Some((_, m)) = self.started.iter_mut().find(|(k, _)| *k == active) else {
            return;
        };
        ui.push_id(active, |ui| m.ui(ui));
        self.last_drawn = Some(active);
    }

    /// The home screen: one button per app.
    fn home(
        &mut self,
        ui: &mut egui::Ui,
        storage: Option<&dyn eframe::Storage>,
        ctx: &egui::Context,
    ) {
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

    /// Persist what the modules keep in eframe's storage.
    ///
    /// This is eframe's autosave, not the exit path: `save_state` is the exit path.
    pub fn save(&mut self, storage: &mut dyn eframe::Storage) {
        for (_, m) in &mut self.started {
            m.save(storage);
        }
    }

    /// The apps that draw their dialogs after the frame, and the ones that ask to quit.
    pub fn after_pass(&mut self, frame: &eframe::Frame) -> bool {
        if self.close_held {
            // The suite held the close because a module has unsaved work. The user must
            // not lose it because an app asked to quit in the same frame.
            self.close_held = false;
            return false;
        }
        let mut quit = false;
        for (_, m) in &mut self.started {
            quit |= m.after_pass(frame);
        }
        quit
    }

    /// Persist every started module's state.
    pub fn save_state(&mut self) {
        for (_, m) in &mut self.started {
            m.save_state();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Color32, Theme};
    use std::cell::Cell;
    use std::rc::Rc;

    /// A module that owns a look of its own, exactly as the real apps do: it puts its
    /// own style and zoom into the context while it runs, and it can change that look
    /// while it is the active one.
    struct Fake {
        kind: Kind,
        /// The colour this module puts into the context. Shared, so a test can change it
        /// the way a user action would.
        color: Rc<Cell<Color32>>,
        zoom: f32,
        font_name: &'static str,
    }

    impl Fake {
        fn new(kind: Kind, color: Color32, zoom: f32, font_name: &'static str) -> Self {
            Self {
                kind,
                color: Rc::new(Cell::new(color)),
                zoom,
                font_name,
            }
        }

        fn install(&self, ctx: &egui::Context) {
            for theme in [Theme::Dark, Theme::Light] {
                let mut visuals = ctx.style_of(theme).visuals.clone();
                visuals.override_text_color = Some(self.color.get());
                ctx.set_visuals_of(theme, visuals);
            }
            ctx.set_zoom_factor(self.zoom);
        }
    }

    impl Module for Fake {
        fn kind(&self) -> Kind {
            self.kind
        }
        fn logic(&mut self, ctx: &egui::Context) {
            self.install(ctx);
        }
        fn raw_input_hook(&mut self, _ctx: &egui::Context, _raw: &mut egui::RawInput) {}
        fn ui(&mut self, _ui: &mut egui::Ui) {}
        fn open_path(&mut self, _path: &str) {}
        fn save_state(&mut self) {}
        fn can_close(&self) -> bool {
            true
        }
        fn fonts(&self) -> egui::FontDefinitions {
            fonts_with(self.font_name)
        }
        fn install_native_menu(&mut self) {}
    }

    /// A font set that differs from the default only by the NAME of one face: the suite
    /// tells font sets apart by their names.
    fn fonts_with(name: &str) -> egui::FontDefinitions {
        let mut defs = egui::FontDefinitions::default();
        if let Some(base) = defs.font_data.values().next().cloned() {
            defs.font_data.insert(name.to_string(), base);
            for family in defs.families.values_mut() {
                family.insert(0, name.to_string());
            }
        }
        defs
    }

    /// A headless frame.
    fn frame(suite: &mut Suite, ctx: &egui::Context) {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 1000.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |root| {
            let ctx = root.ctx().clone();
            suite.logic(&ctx);
            egui::CentralPanel::default().show(root, |ui| {
                suite.ui(ui, None);
            });
        });
        out.textures_delta.clear(); // headless: no renderer to apply them
    }

    /// The colour in force in the context.
    fn color(ctx: &egui::Context) -> Option<Color32> {
        ctx.global_style().visuals.override_text_color
    }

    /// Whether the context is using a font set that holds `name`.
    fn has_font(ctx: &egui::Context, name: &str) -> bool {
        ctx.fonts(|f| f.definitions().font_data.contains_key(name))
    }

    /// A suite with two fake modules: Sheets (blue, 1.25x) and Docs (red, 2x).
    fn two_modules(ctx: &egui::Context) -> (Suite, Rc<Cell<Color32>>) {
        let mut s = Suite::new(Vec::new());
        let sheets = Fake::new(Kind::Sheets, Color32::BLUE, 1.25, "fake-sheets");
        let sheets_color = sheets.color.clone();
        s.activate_module(Kind::Sheets, Box::new(sheets), ctx);
        s.activate_module(
            Kind::Docs,
            Box::new(Fake::new(Kind::Docs, Color32::RED, 2.0, "fake-docs")),
            ctx,
        );
        (s, sheets_color)
    }

    /// Each module keeps its own look: switching shows the other module's look, not the
    /// one the previous module left behind.
    #[test]
    fn each_module_keeps_its_own_look() {
        let ctx = egui::Context::default();
        let (mut s, _) = two_modules(&ctx);

        s.active = Some(Kind::Docs);
        frame(&mut s, &ctx);
        assert_eq!(color(&ctx), Some(Color32::RED), "Docs' own look");

        s.active = Some(Kind::Sheets);
        frame(&mut s, &ctx);
        assert_eq!(color(&ctx), Some(Color32::BLUE), "Sheets' own look");

        s.active = Some(Kind::Docs);
        frame(&mut s, &ctx);
        assert_eq!(
            color(&ctx),
            Some(Color32::RED),
            "Docs' look must come back, not Sheets'"
        );
    }

    /// A runtime change to the active module's own look SURVIVES a switch away and back.
    ///
    /// This is what a construction-time snapshot gets wrong: it would restore the look
    /// the module had when it was built, and throw the user's change away.
    #[test]
    fn a_runtime_look_change_survives_a_switch() {
        let ctx = egui::Context::default();
        let (mut s, sheets_color) = two_modules(&ctx);

        s.active = Some(Kind::Sheets);
        frame(&mut s, &ctx);
        assert_eq!(color(&ctx), Some(Color32::BLUE));

        // The user changes the theme while Sheets is showing.
        sheets_color.set(Color32::from_rgb(0, 255, 0));
        frame(&mut s, &ctx);
        assert_eq!(
            color(&ctx),
            Some(Color32::from_rgb(0, 255, 0)),
            "the change"
        );

        // Away and back: the change must still be there.
        s.active = Some(Kind::Docs);
        frame(&mut s, &ctx);
        assert_eq!(color(&ctx), Some(Color32::RED));
        s.active = Some(Kind::Sheets);
        frame(&mut s, &ctx);
        assert_eq!(
            color(&ctx),
            Some(Color32::from_rgb(0, 255, 0)),
            "the suite reverted the module's own runtime change"
        );
    }

    /// The zoom follows the module that is showing.
    #[test]
    fn the_zoom_follows_the_active_module() {
        let ctx = egui::Context::default();
        let (mut s, _) = two_modules(&ctx);

        s.active = Some(Kind::Docs);
        frame(&mut s, &ctx);
        frame(&mut s, &ctx);
        assert_eq!(ctx.zoom_factor(), 2.0, "Docs' zoom");

        s.active = Some(Kind::Sheets);
        frame(&mut s, &ctx);
        frame(&mut s, &ctx);
        assert_eq!(ctx.zoom_factor(), 1.25, "Sheets' zoom");
    }

    /// The fonts follow the module that is showing.
    #[test]
    fn the_fonts_follow_the_active_module() {
        let ctx = egui::Context::default();
        let (mut s, _) = two_modules(&ctx);

        s.active = Some(Kind::Docs);
        frame(&mut s, &ctx);
        frame(&mut s, &ctx);
        assert!(has_font(&ctx, "fake-docs"), "Docs' fonts");
        assert!(!has_font(&ctx, "fake-sheets"), "not Sheets' fonts");

        s.active = Some(Kind::Sheets);
        frame(&mut s, &ctx);
        frame(&mut s, &ctx);
        assert!(has_font(&ctx, "fake-sheets"), "Sheets' fonts");
        assert!(!has_font(&ctx, "fake-docs"), "not Docs' fonts");
    }
}
