//! Bundled UI fonts, independent of database sort and regional value policies.
use crate::Locale;
use egui::{Context, FontData, FontDefinitions, FontFamily, Id};

const SELAWIK: &str = "Selawik";
const NOTO_JAPANESE: &str = "Noto Sans JP";
const NOTO_KOREAN: &str = "Noto Sans KR";
const NOTO_SIMPLIFIED_CHINESE: &str = "Noto Sans SC";

/// Install Selawik first and regional CJK fallbacks once per locale change.
/// Font selection changes glyph presentation, never stored names or SQL policy.
pub fn configure(ctx: &Context, locale: Locale) {
    let cache_id = Id::new("tablecraft_font_locale");
    if ctx.data(|data| data.get_temp::<Locale>(cache_id)) == Some(locale) {
        return;
    }

    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        (
            SELAWIK,
            include_bytes!("../../../../assets/ui/fonts/selawk.ttf").as_slice(),
        ),
        (
            NOTO_JAPANESE,
            include_bytes!("../../../../assets/ui/fonts/NotoSansJP-Regular.otf").as_slice(),
        ),
        (
            NOTO_KOREAN,
            include_bytes!("../../../../assets/ui/fonts/NotoSansKR-Regular.otf").as_slice(),
        ),
        (
            NOTO_SIMPLIFIED_CHINESE,
            include_bytes!("../../../../assets/ui/fonts/NotoSansSC-Regular.otf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.to_owned(), FontData::from_static(bytes).into());
    }

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, SELAWIK.to_owned());
    let preferred = match locale {
        Locale::Ja => [NOTO_JAPANESE, NOTO_KOREAN, NOTO_SIMPLIFIED_CHINESE],
        Locale::Ko => [NOTO_KOREAN, NOTO_JAPANESE, NOTO_SIMPLIFIED_CHINESE],
        _ => [NOTO_SIMPLIFIED_CHINESE, NOTO_JAPANESE, NOTO_KOREAN],
    };
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        let fallback = fonts.families.entry(family).or_default();
        fallback.extend(preferred.iter().map(|name| (*name).to_owned()));
    }

    ctx.set_fonts(fonts);
    ctx.data_mut(|data| data.insert_temp(cache_id, locale));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_fallbacks_render_japanese_korean_and_simplified_chinese_glyphs() {
        for (locale, text) in [
            (Locale::Ja, "日本語 顧客 名前 東京"),
            (Locale::Ko, "한국어 고객 이름 서울"),
            (Locale::ZhCn, "简体中文 客户 姓名 北京"),
        ] {
            let context = Context::default();
            configure(&context, locale);
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                for family in [FontFamily::Proportional, FontFamily::Monospace] {
                    ui.fonts_mut(|fonts| {
                        // epaint 0.36.2 has_glyph compares face identity with
                        // the replacement face, producing false negatives
                        // for supported glyphs from that same face. Inspect
                        // the real family charmap, then resolve each width.
                        let characters = fonts.fonts.font(&family).characters().clone();
                        for character in text.chars() {
                            assert!(
                                characters.contains_key(&character),
                                "{locale}/{family:?} lacks {character}"
                            );
                            assert!(
                                fonts.glyph_width(
                                    &egui::FontId::new(13.0, family.clone()),
                                    character
                                ) > 0.0
                            );
                        }
                    });
                }
                assert_eq!(
                    ui.fonts_mut(|fonts| {
                        fonts.definitions().families[&FontFamily::Proportional][0].clone()
                    }),
                    SELAWIK
                );
                ui.label(text);
                ui.monospace(text);
            });
            assert!(!output.shapes.is_empty());
            output.textures_delta.clear();
        }
    }
}
