use egui::{Color32, Context, FontId, TextStyle, Visuals};

const DEFAULT_DATASHEET_ZOOM: u16 = 100;
pub(crate) const MIN_DATASHEET_ZOOM: u16 = 50;
pub(crate) const MAX_DATASHEET_ZOOM: u16 = 200;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    System,
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
    fn preference(self) -> egui::ThemePreference {
        match self {
            Self::System => egui::ThemePreference::System,
            Self::Light => egui::ThemePreference::Light,
            Self::Dark => egui::ThemePreference::Dark,
        }
    }
}

fn colors(theme: egui::Theme) -> Palette {
    match theme {
        egui::Theme::Light => Palette {
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
        egui::Theme::Dark => Palette {
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

pub fn palette(ctx: &Context) -> Palette {
    colors(ctx.theme())
}

pub fn set_theme(ctx: &Context, theme: Theme) {
    let preference = theme.preference();
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
    for selected in [egui::Theme::Light, egui::Theme::Dark] {
        let colors = colors(selected);
        let mut style = (*ctx.style_of(selected)).clone();
        style.visuals = match selected {
            egui::Theme::Light => Visuals::light(),
            egui::Theme::Dark => Visuals::dark(),
        };
        style.visuals.override_text_color = Some(colors.ink);
        style.visuals.panel_fill = colors.title_fill;
        style.visuals.window_fill = colors.surface;
        style.visuals.selection.bg_fill = colors.active_row;
        // egui uses this stroke color for selected glyphs as well as outlines.
        style.visuals.selection.stroke.color = colors.ink;
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

pub(crate) fn datasheet_zoom(ctx: &Context) -> u16 {
    ctx.data(|data| data.get_temp::<u16>(egui::Id::new("datasheet_zoom")))
        .unwrap_or(DEFAULT_DATASHEET_ZOOM)
}

pub(crate) fn set_datasheet_zoom(ctx: &Context, percent: u16) {
    let percent = percent.clamp(MIN_DATASHEET_ZOOM, MAX_DATASHEET_ZOOM);
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("datasheet_zoom"), percent));
    ctx.request_repaint();
}

pub(crate) fn datasheet_scale(ctx: &Context) -> f32 {
    f32::from(datasheet_zoom(ctx)) / f32::from(DEFAULT_DATASHEET_ZOOM)
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

    #[test]
    fn system_default_tracks_host_theme_and_explicit_choices_override_it() {
        let context = Context::default();
        configure(&context);
        assert_eq!(Theme::default(), Theme::System);
        for (choice, host, expected) in [
            (Theme::System, egui::Theme::Light, egui::Theme::Light),
            (Theme::System, egui::Theme::Dark, egui::Theme::Dark),
            (Theme::Light, egui::Theme::Dark, egui::Theme::Light),
            (Theme::System, egui::Theme::Dark, egui::Theme::Dark),
        ] {
            set_theme(&context, choice);
            let mut output = context.run_ui(
                egui::RawInput {
                    system_theme: Some(host),
                    ..Default::default()
                },
                |_| {},
            );
            output.textures_delta.clear();
            assert_eq!(context.theme(), expected);
            assert_eq!(
                context.style_of(context.theme()).visuals.dark_mode,
                expected == egui::Theme::Dark
            );
        }
    }

    #[test]
    fn selected_text_paints_normal_ink_while_custom_focus_keeps_its_accent() {
        for theme in [Theme::Light, Theme::Dark] {
            let context = Context::default();
            configure(&context);
            set_theme(&context, theme);
            let mut text = "Selected literal".to_owned();
            let colors = palette(&context);
            let mut selected = None;
            for frame in 0..2 {
                let events = if frame == 1 {
                    vec![egui::Event::Key {
                        key: egui::Key::A,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers {
                            ctrl: true,
                            command: true,
                            ..egui::Modifiers::NONE
                        },
                    }]
                } else {
                    Vec::new()
                };
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(500.0, 100.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        let response = ui.text_edit_singleline(&mut text);
                        if frame == 0 {
                            response.request_focus();
                        }
                        input_border(ui, &response);
                    },
                );
                output.textures_delta.clear();
                if frame == 1 {
                    selected = Some(output);
                }
            }
            let output = selected.unwrap();
            let galley = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(painted) if painted.galley.job.text == text => {
                        Some(&painted.galley)
                    }
                    _ => None,
                })
                .expect("the selected input text is actually painted");
            let vertices: Vec<_> = galley
                .rows
                .iter()
                .flat_map(|row| &row.visuals.mesh.vertices)
                .collect();
            assert!(
                vertices
                    .iter()
                    .any(|vertex| vertex.color == colors.active_row),
                "actual selection background"
            );
            let selected_glyphs_use_ink = vertices.iter().any(|vertex| {
                let is_glyph = vertex.uv != egui::epaint::WHITE_UV;
                is_glyph && vertex.color == colors.ink
            });
            assert!(
                selected_glyphs_use_ink,
                "selected glyphs retain readable ink"
            );
            assert!(!vertices.iter().any(|vertex| vertex.color == colors.accent));
            let accent_focus_ring = output.shapes.iter().any(|shape| match &shape.shape {
                egui::Shape::Rect(rect) => rect.stroke.color == colors.accent,
                _ => false,
            });
            assert!(accent_focus_ring, "custom focus ring retains the accent");
            assert_eq!(text, "Selected literal");
        }
    }
}
