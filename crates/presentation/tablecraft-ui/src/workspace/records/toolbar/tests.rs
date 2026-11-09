use super::*;

#[test]
fn record_position_width_fits_measured_six_and_ten_digit_text_without_unbounded_growth() {
    let context = egui::Context::default();
    crate::configure(&context);
    let mut output = context.run_ui(Default::default(), |ui| {
        let compact = record_position_width(ui, "1", None);
        assert_eq!(compact, POSITION_WIDTH);
        for text in ["100969", "4294967296"] {
            let font = egui::FontSelection::Default.resolve(ui.style());
            let measured = ui
                .painter()
                .layout_no_wrap(text.into(), font, ui.visuals().text_color())
                .size()
                .x;
            let width = record_position_width(ui, text, None);
            assert!(width >= measured + POSITION_TEXT_PADDING);
            assert!(width > compact);
            assert_eq!(
                record_position_width(ui, "1", Some(text.parse().unwrap())),
                width,
                "the known total reserves stable space before reaching its final record"
            );
        }
        assert_eq!(
            record_position_width(ui, "99999999999999999999999", None),
            record_position_width(ui, "9999999999", None)
        );
    });
    output.textures_delta.clear();
}
