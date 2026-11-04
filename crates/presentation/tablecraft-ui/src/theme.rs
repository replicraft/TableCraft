use egui::{Color32, Context, FontData, FontDefinitions, FontFamily, FontId, TextStyle, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(185, 16, 38);
pub const INK: Color32 = Color32::from_rgb(32, 32, 32);
pub const MUTED: Color32 = Color32::from_rgb(96, 96, 96);
pub const TITLE_FILL: Color32 = Color32::from_rgb(240, 240, 240);
pub const GRID_HEADER: Color32 = Color32::from_rgb(225, 228, 232);
pub const GRID_LINE: Color32 = Color32::from_rgb(217, 221, 226);

/// Install bundled Selawik with egui's fallback glyph coverage and custom colors.
pub fn configure(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "Selawik".into(),
        FontData::from_static(include_bytes!("../../../../assets/ui/fonts/selawk.ttf")).into(),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "Selawik".into());
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
    style.visuals = Visuals::light();
    style.visuals.override_text_color = Some(INK);
    style.visuals.panel_fill = TITLE_FILL;
    style.visuals.selection.bg_fill = Color32::from_rgb(166, 205, 234);
    style.visuals.selection.stroke.color = ACCENT;
    style.spacing.item_spacing = egui::vec2(6.0, 3.0);
    style.spacing.button_padding = egui::vec2(7.0, 3.0);
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(13.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(13.0));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(18.0));
    ctx.set_theme(egui::Theme::Light);
    ctx.set_style_of(egui::Theme::Light, style);
}
