use super::*;
use egui::{FontId, RawInput, RichText, Shape, TextEdit};
use harfrust::{FontRef, ShapeOptions, ShaperData, Tag, UnicodeBuffer};
use std::sync::Arc;

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
            for family in [
                FontFamily::Proportional,
                FontFamily::Monospace,
                FontFamily::Name(BOLD_FAMILY.into()),
            ] {
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
                            fonts.glyph_width(&egui::FontId::new(13.0, family.clone()), character)
                                > 0.0
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

#[test]
fn emoji_outlines_render_in_actual_labels_and_monospace_code_editors() {
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        let context = Context::default();
        configure(&context, Locale::EnUs);
        let font_id = FontId::new(14.0, family.clone());
        let mut editor = "🦀".to_owned();
        let mut output = context.run_ui(RawInput::default(), |ui| {
            ui.fonts_mut(|fonts| {
                let candidates = fonts.fonts.font(&family).characters().clone();
                let name = candidates
                    .get(&'🦀')
                    .expect("real crab charmap entry")
                    .first()
                    .unwrap();
                assert_eq!(name, "Noto Emoji");
                let data = &fonts.definitions().font_data[name];
                let face = FontRef::from_index(data.font.as_ref(), data.index).unwrap();
                let cache = ShaperData::new(&face);
                let shaper = cache.shaper(&face).build();
                let shape = |text: &str| {
                    let mut buffer = UnicodeBuffer::new();
                    buffer.push_str(text);
                    buffer.guess_segment_properties();
                    shaper.shape(buffer, ShapeOptions::new())
                };
                let crab = shape("🦀");
                assert_eq!(crab.glyph_infos().len(), 1);
                assert_ne!(
                    crab.glyph_infos()[0].glyph_id,
                    0,
                    "independent native font shaping must not use .notdef"
                );
                assert!(crab.glyph_positions()[0].x_advance > 0);
                let unsupported = shape("\u{0378}");
                assert_eq!(unsupported.glyph_infos().len(), 1);
                assert_eq!(
                    unsupported.glyph_infos()[0].glyph_id,
                    0,
                    "negative control must be unsupported by the actual emoji face"
                );
            });

            if family == FontFamily::Monospace {
                ui.add(
                    TextEdit::multiline(&mut editor)
                        .code_editor()
                        .font(font_id.clone()),
                );
            } else {
                ui.label(RichText::new("🦀").font(font_id.clone()));
            }
            for control in ["\u{fffd}", "\u{0378}"] {
                ui.label(RichText::new(control).font(font_id.clone()));
            }
        });
        let painted = |text: &str| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    Shape::Text(painted) if painted.galley.job.text == text => {
                        Some(painted.galley.clone())
                    }
                    _ => None,
                })
                .expect("actual rendered widget text")
        };
        let crab = painted("🦀");
        assert_eq!(crab.job.sections[0].format.font_id.family, family);
        let glyph = &crab.rows[0].glyphs[0];
        assert!(glyph.advance_width.is_finite() && glyph.advance_width > 0.0);
        assert!(!glyph.uv_rect.is_nothing());
        assert!(!crab.rows[0].visuals.mesh.indices.is_empty());
        for control in ["\u{fffd}", "\u{0378}"] {
            assert_ne!(
                glyph.uv_rect,
                painted(control).rows[0].glyphs[0].uv_rect,
                "real emoji must not reuse replacement artwork"
            );
        }
        let atlas = context.fonts_mut(|fonts| fonts.image());
        let [left, top] = glyph.uv_rect.min.map(usize::from);
        let [right, bottom] = glyph.uv_rect.max.map(usize::from);
        assert!(right <= atlas.size[0] && bottom <= atlas.size[1]);
        assert!(
            (top..bottom)
                .any(|y| (left..right).any(|x| atlas.pixels[y * atlas.size[0] + x].a() > 0)),
            "actual monochrome outline must contain atlas ink"
        );
        assert_eq!(editor, "🦀");
        output.textures_delta.clear();
    }
}

#[test]
fn emoji_fallback_preserves_primary_faces_regional_order_and_locale_cache() {
    for locale in Locale::ALL {
        let context = Context::default();
        configure(&context, locale);
        let mut previous = None;
        for _ in 0..2 {
            let mut output = context.run_ui(RawInput::default(), |ui| {
                configure(ui.ctx(), locale);
                ui.fonts_mut(|fonts| {
                    let definitions = fonts.definitions();
                    assert_eq!(
                        definitions.families[&FontFamily::Proportional][0],
                        "Selawik"
                    );
                    assert_eq!(definitions.families[&FontFamily::Monospace][0], "Hack");
                    let regional = match locale {
                        Locale::Ja => ["Noto Sans JP", "Noto Sans KR", "Noto Sans SC"],
                        Locale::Ko => ["Noto Sans KR", "Noto Sans JP", "Noto Sans SC"],
                        _ => ["Noto Sans SC", "Noto Sans JP", "Noto Sans KR"],
                    };
                    for family in [FontFamily::Proportional, FontFamily::Monospace] {
                        let names = &definitions.families[&family];
                        let emoji = names.iter().position(|name| name == "Noto Emoji").unwrap();
                        for old in ["NotoEmoji-Regular", "emoji-icon-font"] {
                            assert!(emoji < names.iter().position(|name| name == old).unwrap());
                        }
                        assert_eq!(&names[names.len() - 3..], regional);
                    }
                    let current = definitions.font_data["Noto Emoji"].clone();
                    if let Some(previous) = &previous {
                        assert!(
                            Arc::ptr_eq(previous, &current),
                            "same-locale frames must retain their font registration"
                        );
                    }
                    previous = Some(current);
                });
            });
            output.textures_delta.clear();
        }
    }
}

#[test]
fn emoji_order_retains_other_faces_when_legacy_defaults_are_present_or_absent() {
    for (input, expected) in [
        (
            vec![
                "Primary",
                "NotoEmoji-Regular",
                "emoji-icon-font",
                "Regional",
            ],
            vec![
                "Primary",
                "Noto Emoji",
                "NotoEmoji-Regular",
                "emoji-icon-font",
                "Regional",
            ],
        ),
        (
            vec!["Primary", "Regional"],
            vec!["Primary", "Regional", "Noto Emoji"],
        ),
    ] {
        let mut names = input.into_iter().map(str::to_owned).collect();
        insert_emoji_fallback(&mut names);
        assert_eq!(names, expected);
    }
}

#[test]
fn named_bold_family_selects_real_weight_and_retains_locale_fallbacks() {
    for locale in Locale::ALL {
        let context = Context::default();
        configure(&context, locale);
        let family = FontFamily::Name(BOLD_FAMILY.into());
        let mut output = context.run_ui(RawInput::default(), |ui| {
            ui.fonts_mut(|fonts| {
                let characters = fonts.fonts.font(&family).characters().clone();
                let definitions = fonts.definitions();
                let names = &definitions.families[&family];
                assert_eq!(names[0], "Selawik Bold");
                assert_eq!(
                    &names[1..],
                    &definitions.families[&FontFamily::Proportional]
                );

                let latin_face = characters[&'A'].first().unwrap();
                assert_eq!(latin_face, "Selawik Bold");
                let data = &definitions.font_data[latin_face];
                let face = FontRef::from_index(data.font.as_ref(), data.index).unwrap();
                let weight = face
                    .table_data(Tag::new(b"OS/2"))
                    .unwrap()
                    .read_at::<u16>(4)
                    .unwrap();
                assert_eq!(weight, 700, "selected actual face must have bold weight");

                let regular = &definitions.font_data["Selawik"];
                let regular_face =
                    FontRef::from_index(regular.font.as_ref(), regular.index).unwrap();
                assert_eq!(
                    regular_face
                        .table_data(Tag::new(b"OS/2"))
                        .unwrap()
                        .read_at::<u16>(4)
                        .unwrap(),
                    400
                );
                assert_eq!(characters[&'🦀'].first().unwrap(), "Noto Emoji");
                for character in ['日', '한', '简'] {
                    assert!(characters.contains_key(&character));
                    assert_ne!(characters[&character].first().unwrap(), "Selawik Bold");
                }
            });
            ui.label(RichText::new("Tables 日本 한글 简体 🦀").family(family.clone()));
        });
        let painted = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                Shape::Text(text) if text.galley.job.text == "Tables 日本 한글 简体 🦀" => {
                    Some(&text.galley)
                }
                _ => None,
            })
            .expect("actual bold-family widget text");
        assert_eq!(painted.job.sections[0].format.font_id.family, family);
        assert!(!painted.rows[0].visuals.mesh.indices.is_empty());
        output.textures_delta.clear();
    }
}

#[test]
fn cold_widget_uses_bold_after_the_deferred_font_installation() {
    let context = Context::default();
    let mut selections = Vec::new();
    for expected in [
        FontFamily::Proportional,
        FontFamily::Name(BOLD_FAMILY.into()),
    ] {
        let mut output = context.run_ui(RawInput::default(), |ui| {
            configure(ui.ctx(), Locale::EnUs);
            let family = bold_family(ui);
            selections.push(family.clone());
            ui.label(RichText::new("Tables").family(family));
        });
        let painted = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                Shape::Text(text) if text.galley.job.text == "Tables" => Some(&text.galley),
                _ => None,
            })
            .expect("actual cold-start group label");
        assert_eq!(painted.job.sections[0].format.font_id.family, expected);
        assert!(!painted.rows[0].visuals.mesh.indices.is_empty());
        output.textures_delta.clear();
    }
    assert_eq!(
        selections,
        [
            FontFamily::Proportional,
            FontFamily::Name(BOLD_FAMILY.into()),
        ]
    );
}
