use egui::{Color32, Context, FontData, FontDefinitions, FontFamily, FontId, TextStyle, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(151, 42, 57);
pub const INK: Color32 = Color32::from_rgb(40, 45, 54);
pub const MUTED: Color32 = Color32::from_rgb(107, 113, 123);

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
    style.visuals.panel_fill = Color32::from_rgb(247, 248, 250);
    style.visuals.selection.bg_fill = Color32::from_rgb(244, 224, 229);
    style.visuals.selection.stroke.color = ACCENT;
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(24.0));
    ctx.set_theme(egui::Theme::Light);
    ctx.set_style_of(egui::Theme::Light, style);
}
