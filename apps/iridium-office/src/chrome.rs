//! The Workspace brand: tokens, and the chrome that draws itself with them.
//!
//! The suite chrome (the app rail, the document tab strip, the window frame)
//! is one brand, and the seven embedded apps keep their own themes. So the
//! chrome never borrows the active module's style or fonts: it has its own,
//! applied locally.
//!
//! Everything here is paint-level. A tab is a painted surface, not a button
//! with a different fill — that is what made the old strip read as a row of
//! text boxes.

use egui::{
    Align2, Color32, CornerRadius, FontDefinitions, FontFamily, FontId, Id, Painter, Pos2, Rect,
    Response, Sense, Stroke, StrokeKind, Style, Ui, Vec2, pos2, vec2,
};

/// The chrome's font family name, registered once at startup.
pub const FONT_FAMILY: &str = "workspace-ui";

/// Spacing scale, in points.
pub mod space {
    /// 4 points.
    pub const XS: f32 = 4.0;
    /// 8 points.
    pub const S: f32 = 8.0;
    /// 12 points.
    pub const M: f32 = 12.0;
    /// 16 points.
    pub const L: f32 = 16.0;
    /// 24 points.
    pub const XL: f32 = 24.0;
    /// 32 points.
    pub const XXL: f32 = 32.0;
}

/// Type scale, in points.
pub mod text {
    /// The window and app title.
    pub const TITLE: f32 = 20.0;
    /// A section heading.
    pub const HEADING: f32 = 18.0;
    /// A document tab's filename.
    pub const TAB: f32 = 13.0;
    /// Body text.
    pub const BODY: f32 = 14.0;
    /// A rail label, a caption, a status line.
    pub const SMALL: f32 = 12.0;
    /// The product monogram.
    pub const MONOGRAM: f32 = 18.0;
}

/// The colours the chrome draws with.
///
/// One struct for both themes: the values change, the names do not.
#[derive(Clone, Debug)]
pub struct Chrome {
    /// The brand colour: the rail's active bar, the window title.
    pub brand: Color32,
    /// Focus rings and links.
    pub accent: Color32,
    /// The window behind everything.
    pub window: Color32,
    /// The rail's own background.
    pub rail: Color32,
    /// The active rail item.
    pub selected: Color32,
    /// The tab strip's background.
    pub strip: Color32,
    /// The active tab, and a home card.
    pub tab: Color32,
    /// The area behind an embedded app's document.
    pub canvas: Color32,
    /// A hovered surface.
    pub hover: Color32,
    /// A pressed surface.
    pub pressed: Color32,
    /// A separator or border.
    pub separator: Color32,
    /// Primary text.
    pub text: Color32,
    /// Secondary or muted text.
    pub muted: Color32,
    /// Text on a solid brand or app tile.
    pub on_accent: Color32,
}

impl Chrome {
    /// The light theme.
    pub fn light() -> Self {
        Self {
            brand: rgb(0x0B6E4F),
            accent: rgb(0x2F6FEB),
            window: rgb(0xF5F6F8),
            rail: rgb(0xEEF1F4),
            selected: rgb(0xE1EEE8),
            strip: rgb(0xE9EDF1),
            tab: rgb(0xFFFFFF),
            canvas: rgb(0xFFFFFF),
            hover: rgb(0xE2E7EC),
            pressed: rgb(0xD5DDE5),
            separator: rgb(0xC7D0D8),
            text: rgb(0x1E2933),
            muted: rgb(0x52616D),
            on_accent: rgb(0xFFFFFF),
        }
    }

    /// The dark theme.
    pub fn dark() -> Self {
        Self {
            brand: rgb(0x4FB389),
            accent: rgb(0x8BB4FF),
            window: rgb(0x171B20),
            rail: rgb(0x11161B),
            selected: rgb(0x20382F),
            strip: rgb(0x14191F),
            tab: rgb(0x252D35),
            canvas: rgb(0x1C2229),
            hover: rgb(0x2A3440),
            pressed: rgb(0x344252),
            separator: rgb(0x3B4754),
            text: rgb(0xE8EDF2),
            muted: rgb(0xAAB6C2),
            on_accent: rgb(0x10251C),
        }
    }

    /// The chrome colours for the context's current theme.
    pub fn for_theme(dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }

    /// An egui style that draws the chrome, independent of the active module.
    ///
    /// Built from `Style::default`, never from the module's own style: the
    /// module's theme belongs to the module.
    pub fn style(&self) -> Style {
        let mut style = Style::default();
        let mut visuals = if self.is_dark() {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.panel_fill = self.window;
        visuals.window_fill = self.tab;
        visuals.extreme_bg_color = self.canvas;
        visuals.faint_bg_color = self.rail;
        visuals.selection.bg_fill = self.selected;
        visuals.selection.stroke = Stroke::new(1.0, self.brand);
        visuals.override_text_color = Some(self.text);
        visuals.hyperlink_color = self.accent;
        visuals.warn_fg_color = rgb(0x92601B);
        visuals.error_fg_color = rgb(0xB63743);
        visuals.widgets.noninteractive.bg_fill = self.window;
        visuals.widgets.noninteractive.weak_bg_fill = self.window;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.separator);
        visuals.widgets.inactive.bg_fill = self.tab;
        visuals.widgets.inactive.weak_bg_fill = self.tab;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, self.separator);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.hovered.bg_fill = self.hover;
        visuals.widgets.hovered.weak_bg_fill = self.hover;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, self.separator);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.active.bg_fill = self.pressed;
        visuals.widgets.active.weak_bg_fill = self.pressed;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, self.accent);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.open.bg_fill = self.selected;
        visuals.widgets.open.weak_bg_fill = self.selected;
        visuals.widgets.open.bg_stroke = Stroke::new(1.0, self.brand);
        visuals.widgets.open.fg_stroke = Stroke::new(1.0, self.text);
        style.visuals = visuals;
        style
    }

    /// Whether this theme is the dark one.
    pub fn is_dark(&self) -> bool {
        // The dark theme's window colour is the giveaway; comparing the two
        // built-in themes is simpler and cannot drift.
        self.window == Self::dark().window
    }
}

/// A colour from a 24-bit hex value.
fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

/// The chrome's font at a given size.
pub fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(FONT_FAMILY.into()))
}

/// Register the chrome's own font family, once, before the first frame.
///
/// The suite's chrome must not change when an embedded app swaps its fonts at
/// runtime, so the chrome has its own named family. A font provider survives
/// those swaps.
pub fn install_font(ctx: &egui::Context) {
    let base = FontDefinitions::default();
    let mut chrome = FontDefinitions::default();
    chrome.font_data.clear();
    chrome.families.clear();
    for (name, data) in &base.font_data {
        chrome
            .font_data
            .insert(format!("workspace/{name}"), data.clone());
    }
    let chain = base.families[&FontFamily::Proportional]
        .iter()
        .map(|name| format!("workspace/{name}"))
        .collect();
    chrome
        .families
        .insert(FontFamily::Name(FONT_FAMILY.into()), chain);
    ctx.add_font_provider(std::sync::Arc::new(chrome));
}

/// A solid app-identity tile with a two-letter mark.
pub fn accent_tile(painter: &Painter, rect: Rect, mark: &str, accent: Color32, on_accent: Color32) {
    painter.rect_filled(rect, CornerRadius::same(6), accent);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        mark,
        font(12.0),
        on_accent,
    );
}

/// A keyboard focus ring, drawn inside the rectangle it belongs to.
pub fn focus_ring(painter: &Painter, rect: Rect, color: Color32) {
    painter.rect_stroke(
        rect.shrink(1.0),
        CornerRadius::same(8),
        Stroke::new(2.0, color),
        StrokeKind::Inside,
    );
}

/// A rail item: an app tile, its label, and the state that marks it active.
#[allow(clippy::too_many_arguments)]
pub fn rail_item(
    ui: &mut Ui,
    id: Id,
    label: &str,
    mark: &str,
    accent: Color32,
    active: bool,
    running: bool,
    chrome: &Chrome,
) -> Response {
    let (_, rect) = ui.allocate_space(vec2(68.0, 68.0));
    let response = ui.interact(rect, id, Sense::click());

    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, active, label));

    let painter = ui.painter();
    let fill = if response.is_pointer_button_down_on() {
        chrome.pressed
    } else if active {
        chrome.selected
    } else if response.hovered() {
        chrome.hover
    } else {
        Color32::TRANSPARENT
    };
    painter.rect_filled(rect, CornerRadius::same(10), fill);

    if active {
        // The brand-coloured bar that says which app this is.
        let bar = Rect::from_min_size(rect.min + vec2(0.0, 12.0), vec2(3.0, 44.0));
        painter.rect_filled(bar, CornerRadius::same(1), chrome.brand);
    }

    let tile = Rect::from_min_size(rect.min + vec2(20.0, 8.0), vec2(28.0, 28.0));
    accent_tile(painter, tile, mark, accent, chrome.on_accent);

    if running {
        // A running app carries a dot; one that has never started does not.
        painter.circle_filled(tile.right_top() + vec2(5.0, 0.0), 2.0, chrome.muted);
    }

    painter.text(
        rect.min + vec2(34.0, 52.0),
        Align2::CENTER_CENTER,
        label,
        font(text::SMALL),
        chrome.text,
    );

    if response.has_focus() {
        focus_ring(painter, rect, chrome.accent);
    }

    response.on_hover_text(if running {
        format!("{label} · Running")
    } else {
        format!("{label} · Not running")
    })
}

/// A document tab: an app tile, the filename, and a close mark inside it.
///
/// Returns the activate and close responses separately. Their hit areas do
/// not overlap, which is what stops a click on the cross from also switching
/// to the document.
#[allow(clippy::too_many_arguments)]
pub fn document_tab(
    ui: &mut Ui,
    id: Id,
    filename: &str,
    full_path: &str,
    mark: &str,
    accent: Color32,
    active: bool,
    width: f32,
    chrome: &Chrome,
) -> (Response, Response) {
    let (_, rect) = ui.allocate_space(vec2(width.clamp(160.0, 240.0), 36.0));

    let close_rect = Rect::from_min_size(
        pos2(rect.right() - 30.0, rect.top() + 6.0),
        vec2(24.0, 24.0),
    );
    let body_rect = Rect::from_min_max(rect.min, pos2(close_rect.left(), rect.bottom()));
    let body = ui.interact(body_rect, id, Sense::click());
    let close = ui.interact(close_rect, id.with("close"), Sense::click());

    body.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, active, filename));
    close.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Close {filename}"))
    });

    let painter = ui.painter();
    let hovered = body.hovered() || close.hovered();
    let fill = if body.is_pointer_button_down_on() {
        chrome.pressed
    } else if active {
        chrome.tab
    } else if hovered {
        chrome.hover
    } else {
        Color32::TRANSPARENT
    };
    // Top-rounded, flush with the bottom of the strip: a tab, not a card.
    painter.rect_filled(
        rect,
        CornerRadius {
            nw: 8,
            ne: 8,
            sw: 0,
            se: 0,
        },
        fill,
    );

    if active {
        // The owning app's colour along the top edge.
        painter.line_segment(
            [
                pos2(rect.left() + 8.0, rect.top() + 1.0),
                pos2(rect.right() - 8.0, rect.top() + 1.0),
            ],
            Stroke::new(2.0, accent),
        );
    }

    let tile = Rect::from_min_size(rect.min + vec2(12.0, 8.0), vec2(20.0, 20.0));
    accent_tile(painter, tile, mark, accent, chrome.on_accent);

    let text_rect = Rect::from_min_max(
        rect.min + vec2(40.0, 0.0),
        pos2(close_rect.left() - 4.0, rect.bottom()),
    );
    painter
        .with_clip_rect(text_rect.intersect(ui.clip_rect()))
        .text(
            text_rect.left_center(),
            Align2::LEFT_CENTER,
            filename,
            font(text::TAB),
            if active { chrome.text } else { chrome.muted },
        );

    if close.hovered() || close.is_pointer_button_down_on() {
        painter.rect_filled(
            close_rect,
            CornerRadius::same(4),
            if close.is_pointer_button_down_on() {
                chrome.pressed
            } else {
                chrome.hover
            },
        );
    }
    // The close mark: two strokes inside the tab, not a button beside it.
    let center = close_rect.center();
    for direction in [-1.0_f32, 1.0_f32] {
        let delta = vec2(4.0, 4.0 * direction);
        painter.line_segment(
            [center - delta, center + delta],
            Stroke::new(1.5, chrome.muted),
        );
    }

    if body.has_focus() {
        focus_ring(painter, rect, chrome.accent);
    }
    if close.has_focus() {
        focus_ring(painter, close_rect, chrome.accent);
    }

    (
        body.on_hover_text(format!("{filename}\n{full_path}")),
        close.on_hover_text(format!("Close {filename}")),
    )
}

/// A home-screen card for one app.
pub fn app_card(
    ui: &mut Ui,
    id: Id,
    label: &str,
    mark: &str,
    accent: Color32,
    chrome: &Chrome,
) -> Response {
    let (_, rect) = ui.allocate_space(vec2(160.0, 112.0));
    let response = ui.interact(rect, id, Sense::click());

    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));

    let painter = ui.painter();
    let fill = if response.is_pointer_button_down_on() {
        chrome.pressed
    } else if response.hovered() {
        chrome.hover
    } else {
        chrome.tab
    };
    painter.rect_filled(rect, CornerRadius::same(12), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(12),
        Stroke::new(1.0, chrome.separator),
        StrokeKind::Inside,
    );

    let tile = Rect::from_min_size(rect.min + vec2(16.0, 16.0), vec2(32.0, 32.0));
    accent_tile(painter, tile, mark, accent, chrome.on_accent);

    painter.text(
        rect.min + vec2(16.0, 68.0),
        Align2::LEFT_CENTER,
        label,
        font(text::BODY),
        chrome.text,
    );

    if response.has_focus() {
        focus_ring(painter, rect, chrome.accent);
    }

    response
}

/// Where a point in a painted rect lands, as a fraction from its top left.
pub fn relative_position(rect: Rect, point: Pos2) -> Vec2 {
    vec2(
        (point.x - rect.left()) / rect.width(),
        (point.y - rect.top()) / rect.height(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two themes differ, and both are readable: text on its own surface
    /// clears the contrast floor for body text.
    #[test]
    fn both_themes_have_readable_text() {
        for chrome in [Chrome::light(), Chrome::dark()] {
            assert!(
                contrast(chrome.text, chrome.window) >= 4.5,
                "text on the window"
            );
            assert!(contrast(chrome.text, chrome.tab) >= 4.5, "text on a tab");
            assert!(
                contrast(chrome.muted, chrome.strip) >= 4.5,
                "muted text on the strip"
            );
            assert!(
                contrast(chrome.on_accent, chrome.brand) >= 4.5,
                "text on the brand"
            );
        }
    }

    /// The light and dark themes are genuinely different themes.
    #[test]
    fn the_themes_differ() {
        assert_ne!(Chrome::light().window, Chrome::dark().window);
        assert!(Chrome::light().is_dark() == false);
        assert!(Chrome::dark().is_dark());
    }

    /// The WCAG relative-luminance contrast ratio between two colours.
    fn contrast(a: Color32, b: Color32) -> f32 {
        let l = |c: Color32| {
            let f = |v: u8| {
                let v = v as f32 / 255.0;
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
        };
        let (la, lb) = (l(a), l(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }
}
