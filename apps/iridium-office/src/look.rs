//! One app's look in the shared egui context.
//!
//! Five apps share ONE egui context, and each of them replaces that context's fonts,
//! styles and zoom with its own. [`Look`] is what one app left behind, so the suite can
//! put it back when that app is the one on screen.
//!
//! The look is captured LIVE, never replayed from a construction-time snapshot: a
//! snapshot would revert the app's own runtime changes (a theme the user picked, an
//! interface-size change) and would drop fonts the app has since added (PDF's
//! `pdf-source-*` document faces, Sheets' workbook fonts).

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{FontDefinitions, FontFamily, Style, Theme, ThemePreference};

/// The names of the faces in a font set, and the families they fill.
///
/// A cheap fingerprint: comparing the font DATA would mean comparing every TTF, and the
/// names are enough to tell one app's set from another's.
pub type FontSig = (Vec<String>, BTreeMap<FontFamily, Vec<String>>);

/// What one app installs into the shared context.
#[derive(Clone)]
pub struct Look {
    /// The font set, including any face the app added while running.
    pub fonts: FontDefinitions,
    /// The fingerprint of `fonts`.
    pub font_sig: FontSig,
    /// The style the app uses in the dark theme.
    pub dark: Arc<Style>,
    /// The style the app uses in the light theme.
    pub light: Arc<Style>,
    /// Which theme the app follows.
    pub preference: ThemePreference,
    /// Which theme to use when the preference follows the system and the system is silent.
    pub fallback: Theme,
    /// The interface zoom factor.
    pub zoom: f32,
}

impl Look {
    /// Read the look the context holds now.
    ///
    /// `fonts` is passed in because the context cannot be read back before egui's first
    /// pass: `Context::fonts` panics with "No fonts available until first call to
    /// Context::run()". The caller knows the set the app built.
    pub fn capture(ctx: &egui::Context, fonts: FontDefinitions) -> Self {
        let font_sig = Self::sig_of(&fonts);
        Self {
            fonts,
            font_sig,
            dark: ctx.style_of(Theme::Dark),
            light: ctx.style_of(Theme::Light),
            preference: ctx.options(|o| o.theme_preference),
            fallback: ctx.options(|o| o.fallback_theme),
            zoom: ctx.zoom_factor(),
        }
    }

    /// Read the context's current styles and theme, keeping the fonts and the zoom.
    pub fn capture_styles(&mut self, ctx: &egui::Context) {
        self.dark = ctx.style_of(Theme::Dark);
        self.light = ctx.style_of(Theme::Light);
        self.preference = ctx.options(|o| o.theme_preference);
        self.fallback = ctx.options(|o| o.fallback_theme);
    }

    /// Put the styles and the theme back into the context.
    ///
    /// Unlike fonts and zoom, these apply at once: `set_style_of` writes the options
    /// directly, with nothing pending.
    pub fn restore_styles(&self, ctx: &egui::Context) {
        ctx.set_style_of(Theme::Dark, self.dark.clone());
        ctx.set_style_of(Theme::Light, self.light.clone());
        ctx.set_theme(self.preference);
        ctx.options_mut(|o| o.fallback_theme = self.fallback);
    }

    /// Put the fonts back into the context.
    ///
    /// `Context::set_fonts` compares the definitions first and only rebuilds the font
    /// atlas when they differ, so this is cheap while the fonts are already in place.
    pub fn restore_fonts(&self, ctx: &egui::Context) {
        ctx.set_fonts(self.fonts.clone());
    }

    /// Put the zoom factor in force.
    ///
    /// egui's own `set_zoom_factor` queues the change for the next pass and asks for a
    /// repaint; the suite sets the factor directly instead, because it re-issues it every
    /// frame and a repaint per frame would spin the window at full speed. The apps keep
    /// their own pending change, which egui applies at the next pass as usual.
    pub fn set_zoom(&self, ctx: &egui::Context) {
        ctx.options_mut(|o| o.zoom_factor = self.zoom);
    }

    /// The fingerprint of a font set.
    pub fn sig_of(fonts: &FontDefinitions) -> FontSig {
        (
            fonts.font_data.keys().cloned().collect(),
            fonts.families.clone(),
        )
    }

    /// The fingerprint of the fonts the context is applying.
    pub fn sig_now(ctx: &egui::Context) -> FontSig {
        ctx.fonts(|f| Self::sig_of(f.definitions()))
    }

    /// The fonts the context is applying.
    pub fn fonts_now(ctx: &egui::Context) -> FontDefinitions {
        ctx.fonts(|f| f.definitions().clone())
    }

    /// Whether `sig` still holds every face this look has, and may hold more.
    ///
    /// The app may have ADDED faces since (PDF's document faces), which is how the app's
    /// own additions are told apart from another app's set. The check was the other way
    /// round, so a face the app added while drawing was dropped again and a face it had
    /// lost was kept.
    pub fn covers(&self, sig: &FontSig) -> bool {
        self.font_sig.0.iter().all(|name| sig.0.contains(name))
    }
}
