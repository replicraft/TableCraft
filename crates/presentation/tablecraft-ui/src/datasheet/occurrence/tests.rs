use super::*;
use crate::datasheet::{cell_with_occurrence, preview};
use crate::{DisplayControl, HorizontalAlignment, Theme};
use egui::{Context, Event, OutputCommand, RawInput, Shape, Vec2};
use tablecraft_display::{NativeFormat, ScalarPresentation};

fn occurrence(start_utf16: u32, end_utf16: u32) -> Occurrence {
    Occurrence {
        row: 0,
        column: 0,
        start_utf16,
        end_utf16,
        formatted: false,
    }
}

#[test]
fn utf16_endpoints_require_exact_nonempty_scalar_boundaries() {
    for (text, start, end, expected) in [
        ("a🦀b", 1, 3, Some(1..5)),
        ("a🦀b", 1, 2, None),
        ("a🦀b", 2, 3, None),
        ("a🦀b", 3, 4, Some(5..6)),
        ("e\u{301}x", 1, 2, Some(1..3)),
        ("a…", 1, 2, Some(1..4)),
        ("abc", 2, 2, None),
        ("abc", 3, 2, None),
        ("abc", 0, 4, None),
        ("", 0, 1, None),
    ] {
        assert_eq!(
            utf8_range(text, start, end),
            expected,
            "{text:?} {start}..{end}"
        );
    }
}

#[test]
fn bounded_preview_never_turns_ellipsis_badges_or_unrelated_carriers_into_matches() {
    let cell = Cell::scalar("a".repeat(1_000_000));
    let bounded = preview::text(&cell.text);
    assert_eq!(
        occurrence(511, 512).visible_range(&cell, &bounded),
        Some(511..512)
    );
    for (start, end) in [(512, 513), (511, 513), (999_999, 1_000_000)] {
        assert_eq!(occurrence(start, end).visible_range(&cell, &bounded), None);
    }

    let mut stored = Cell::scalar("abc");
    stored.kind = CellKind::Scalar {
        is_null: false,
        stored_calculation: true,
        boolean: None,
        display_control: DisplayControl::Absent,
    };
    assert_eq!(
        occurrence(1, 3).visible_range(&stored, &preview::text(&stored.text)),
        Some(1..3)
    );
    assert_eq!(
        occurrence(3, 5).visible_range(&stored, &preview::text(&stored.text)),
        None
    );

    for kind in [
        CellKind::Scalar {
            is_null: true,
            stored_calculation: false,
            boolean: None,
            display_control: DisplayControl::Absent,
        },
        CellKind::Scalar {
            is_null: false,
            stored_calculation: false,
            boolean: Some(true),
            display_control: DisplayControl::Native(106),
        },
        CellKind::Attachments {
            assigned: true,
            members: 1,
        },
    ] {
        let cell = Cell {
            kind,
            ..Cell::scalar("abc")
        };
        assert_eq!(
            occurrence(0, 1).visible_range(&cell, &preview::text(&cell.text)),
            None
        );
    }

    let typed = Cell {
        presentation: Some(ScalarPresentation::Currency {
            scaled: 10_000,
            native_format: NativeFormat::Currency,
        }),
        ..Cell::scalar("€1.00")
    };
    let mut found = occurrence(1, 2);
    assert_eq!(
        found.visible_range(&typed, &preview::text(&typed.text)),
        None
    );
    found.formatted = true;
    assert_eq!(
        found.visible_range(&typed, &preview::text(&typed.text)),
        Some(1..2)
    );
}

#[test]
fn highlighted_cell_preserves_galley_widget_focus_and_complete_clipboard_in_both_themes() {
    for theme in [Theme::Light, Theme::Dark] {
        for alignment in [HorizontalAlignment::Left, HorizontalAlignment::Right] {
            for (text, start, end) in [("a🦀b", 1, 3), ("a日本語b", 2, 3)] {
                let context = Context::default();
                context.enable_accesskit();
                crate::configure(&context);
                crate::theme::set_theme(&context, theme);
                let cell = Cell {
                    alignment,
                    ..Cell::scalar(text)
                };
                let input = |events| RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 100.0))),
                    events,
                    ..Default::default()
                };
                let mut id = None;
                let mut original = context.run_ui(input(Vec::new()), |ui| {
                    ui.set_max_size(Vec2::new(200.0, 24.0));
                    let response = cell_with_occurrence(ui, &cell, "Value", 1, true, None);
                    response.request_focus();
                    id = Some(response.id);
                });
                original.textures_delta.clear();
                let mut highlighted = context.run_ui(input(vec![Event::Copy]), |ui| {
                    ui.set_max_size(Vec2::new(200.0, 24.0));
                    let response = cell_with_occurrence(
                        ui,
                        &cell,
                        "Value",
                        1,
                        true,
                        Some(&occurrence(start, end)),
                    );
                    assert_eq!(Some(response.id), id);
                    assert!(response.has_focus());
                });
                highlighted.textures_delta.clear();
                let galley = |output: &egui::FullOutput| {
                    output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            Shape::Text(text) if text.galley.job.text == cell.text => {
                                Some((text.pos, text.galley.clone()))
                            }
                            _ => None,
                        })
                        .expect("unchanged cell text")
                };
                assert_eq!(galley(&original), galley(&highlighted));
                assert!(
                    highlighted
                        .platform_output
                        .commands
                        .contains(&OutputCommand::CopyText(cell.text.clone()))
                );
                let fill = crate::theme::palette(&context).active_header;
                assert_eq!(
                    highlighted
                        .shapes
                        .iter()
                        .filter(|shape| {
                            matches!(&shape.shape, Shape::Rect(rectangle) if rectangle.fill == fill)
                        })
                        .count(),
                    1
                );
            }
        }
    }
}

#[test]
fn geometry_follows_existing_multiline_glyphs_and_excludes_layout_ellipsis() {
    let context = Context::default();
    let mut output = context.run_ui(RawInput::default(), |ui| {
        let galley = ui.painter().layout_no_wrap(
            "a\nbc".into(),
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
        let found = rectangles(&galley, 2..4);
        assert_eq!(found.len(), 2);
        assert!(
            found
                .iter()
                .all(|rectangle| rectangle.top() == galley.rows[1].pos.y)
        );
        let galley = egui::WidgetText::from("abcdefghijk").into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            25.0,
            egui::TextStyle::Body,
        );
        assert!(galley.elided);
        assert_eq!(
            rectangles(&galley, 0..11).len(),
            galley.rows[0].glyphs.len() - 1
        );
    });
    output.textures_delta.clear();
}

#[test]
fn rtl_palindromes_and_repeated_runs_never_claim_logical_occurrence_mapping() {
    let context = Context::default();
    crate::configure(&context);
    let mut output = context.run_ui(RawInput::default(), |ui| {
        for text in ["אבא", "אאא", "سسس", "a אבא b", "\u{202e}aba\u{202c}"] {
            let galley = ui.painter().layout_no_wrap(
                text.into(),
                egui::FontId::proportional(14.0),
                egui::Color32::WHITE,
            );
            assert!(
                rectangles(&galley, 0..1).is_empty(),
                "unqualified directional mapping {text:?}"
            );
        }
    });
    output.textures_delta.clear();
}

#[test]
fn real_ligature_or_emoji_cluster_requires_its_complete_source_span() {
    let context = Context::default();
    crate::configure(&context);
    let mut clusters = 0;
    let mut output = context.run_ui(RawInput::default(), |ui| {
        for text in ["fi", "ffi", "👩\u{200d}💻", "🇺🇸"] {
            let galley = ui.painter().layout_no_wrap(
                text.into(),
                egui::FontId::proportional(14.0),
                egui::Color32::WHITE,
            );
            let glyphs = &galley.rows[0].glyphs;
            let scalars = text.chars().count();
            if glyphs.len() != scalars
                || !glyphs.iter().map(|glyph| glyph.chr).eq(text.chars())
                || glyphs[0].advance_width <= 0.0
                || !glyphs[1..].iter().all(|glyph| glyph.advance_width == 0.0)
            {
                continue;
            }
            clusters += 1;
            assert_eq!(rectangles(&galley, 0..scalars).len(), 1);
            assert!(
                rectangles(&galley, 0..1).is_empty(),
                "partial first scalar of {text:?}"
            );
            assert!(
                rectangles(&galley, 1..scalars).is_empty(),
                "continuations of {text:?}"
            );

            let mixed = format!("a{text}z");
            let mixed_galley = ui.painter().layout_no_wrap(
                mixed,
                egui::FontId::proportional(14.0),
                egui::Color32::WHITE,
            );
            assert_eq!(rectangles(&mixed_galley, 0..scalars + 2).len(), 3);
            assert!(
                rectangles(&mixed_galley, 0..2).is_empty(),
                "ordinary prefix plus partial cluster"
            );
            assert!(
                rectangles(&mixed_galley, scalars..scalars + 2).is_empty(),
                "partial cluster plus ordinary suffix"
            );
        }
    });
    output.textures_delta.clear();
    assert!(
        clusters > 0,
        "configured fonts must actually form a multi-scalar ligature/emoji glyph"
    );
}
