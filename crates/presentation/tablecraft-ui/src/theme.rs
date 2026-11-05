use egui::{Color32, Context, FontId, TextStyle, Visuals};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

/// Semantic workspace colors selected together for the effective theme.
#[derive(Clone, Copy)]
pub struct Palette {
    pub accent: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub title_fill: Color32,
    pub surface: Color32,
    pub grid_header: Color32,
    pub grid_line: Color32,
    pub active_header: Color32,
    pub active_row: Color32,
    pub active_cell: Color32,
    pub checkbox_accent: Color32,
}

impl Theme {
    fn egui(self) -> egui::Theme {
        match self {
            Self::Light => egui::Theme::Light,
            Self::Dark => egui::Theme::Dark,
        }
    }

    fn colors(self) -> Palette {
        match self {
            Self::Light => Palette {
                accent: Color32::from_rgb(185, 16, 38),
                ink: Color32::from_rgb(32, 32, 32),
                muted: Color32::from_rgb(96, 96, 96),
                title_fill: Color32::from_rgb(240, 240, 240),
                surface: Color32::WHITE,
                grid_header: Color32::from_rgb(225, 228, 232),
                grid_line: Color32::from_rgb(217, 221, 226),
                active_header: Color32::from_rgb(255, 225, 102),
                active_row: Color32::from_rgb(166, 205, 234),
                active_cell: Color32::WHITE,
                checkbox_accent: Color32::from_rgb(44, 107, 159),
            },
            Self::Dark => Palette {
                accent: Color32::from_rgb(244, 139, 157),
                ink: Color32::from_rgb(235, 237, 240),
                muted: Color32::from_rgb(166, 172, 182),
                title_fill: Color32::from_rgb(36, 38, 43),
                surface: Color32::from_rgb(27, 29, 33),
                grid_header: Color32::from_rgb(48, 52, 59),
                grid_line: Color32::from_rgb(66, 71, 80),
                active_header: Color32::from_rgb(108, 82, 27),
                active_row: Color32::from_rgb(37, 68, 94),
                active_cell: Color32::from_rgb(31, 34, 39),
                checkbox_accent: Color32::from_rgb(115, 184, 236),
            },
        }
    }
}

pub fn palette(ctx: &Context) -> Palette {
    match ctx.theme() {
        egui::Theme::Light => Theme::Light.colors(),
        egui::Theme::Dark => Theme::Dark.colors(),
    }
}

pub fn set_theme(ctx: &Context, theme: Theme) {
    let preference = egui::ThemePreference::from(theme.egui());
    if ctx.options(|options| options.theme_preference) == preference {
        return;
    }
    ctx.set_theme(preference);
    ctx.request_repaint();
}

/// Install shared assets and both styles without resetting the selected theme.
pub fn configure(ctx: &Context) {
    crate::fonts::configure(ctx, crate::Locale::EnUs);
    egui_extras::install_image_loaders(ctx);
    for theme in [Theme::Light, Theme::Dark] {
        let selected = theme.egui();
        let colors = theme.colors();
        let mut style = (*ctx.style_of(selected)).clone();
        style.visuals = match theme {
            Theme::Light => Visuals::light(),
            Theme::Dark => Visuals::dark(),
        };
        style.visuals.override_text_color = Some(colors.ink);
        style.visuals.panel_fill = colors.title_fill;
        style.visuals.window_fill = colors.surface;
        style.visuals.selection.bg_fill = colors.active_row;
        style.visuals.selection.stroke.color = colors.accent;
        style.visuals.widgets.inactive.corner_radius = 2.into();
        style.visuals.widgets.hovered.corner_radius = 2.into();
        style.visuals.widgets.active.corner_radius = 2.into();
        style.spacing.item_spacing = egui::vec2(6.0, 3.0);
        style.spacing.button_padding = egui::vec2(5.0, 2.0);
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(13.0));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(13.0));
        style
            .text_styles
            .insert(TextStyle::Heading, FontId::proportional(18.0));
        ctx.set_style_of(selected, style);
    }
}

/// Keep editable fields visible against both light and dark surfaces.
pub(crate) fn input_border(ui: &egui::Ui, response: &egui::Response) {
    let colors = palette(ui.ctx());
    let color = if response.enabled() && response.has_focus() {
        colors.accent
    } else {
        colors.grid_line
    };
    ui.painter().rect_stroke(
        response.rect,
        2.0,
        egui::Stroke::new(1.0, color),
        egui::StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_and_style_configuration_preserves_the_selected_dark_theme() {
        let context = Context::default();
        set_theme(&context, Theme::Dark);
        configure(&context);
        assert_eq!(context.theme(), egui::Theme::Dark);
        assert!(context.style_of(context.theme()).visuals.dark_mode);
        set_theme(&context, Theme::Light);
        assert_eq!(context.theme(), egui::Theme::Light);
        assert!(!context.style_of(context.theme()).visuals.dark_mode);
    }
}
