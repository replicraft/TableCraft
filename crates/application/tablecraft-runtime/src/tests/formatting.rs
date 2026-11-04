//! Real stored Format declarations, using existing independent raw fixture helpers.
use super::{request, stored::opened};
use crate::{CellKind, Command, DisplayControl, Response};
use tablecraft_testkit::fixtures::access::{field_presentation, wide_properties};

fn source(format: Option<(&str, u8, u8)>, control: i16, scope: u16) -> Vec<u8> {
    let control = field_presentation::property(1, 1, 3, &control.to_le_bytes());
    let formatting = format.map(|(text, flags, kind)| {
        field_presentation::property(0, flags, kind, &wide_properties::text(text))
    });
    let mut blocks = vec![wide_properties::names(&["Format", "DisplayControl"])];
    match (formatting, scope) {
        (Some(format), 1) => blocks.push(field_presentation::score_group(1, &[format, control])),
        (Some(format), scope) => {
            blocks.push(field_presentation::score_group(scope, &[format]));
            blocks.push(field_presentation::score_group(1, &[control]));
        }
        (None, _) => blocks.push(field_presentation::score_group(1, &[control])),
    }
    field_presentation::database(&blocks, true)
}

#[test]
fn explicit_boolean_text_box_formats_use_english_labels_and_preserve_native_values() {
    for (format, expected) in [
        ("Yes/No", ["No", "Yes"]),
        ("True/False", ["False", "True"]),
        ("on/off", ["Off", "On"]),
    ] {
        let bytes = source(Some((format, 1, 10)), 109, 1);
        let (_directory, path, worker, _revision) = opened(&bytes);
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Items".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected formatted native rows");
        };
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row[1].text, expected[index]);
            assert_eq!(
                row[1].kind,
                CellKind::Scalar {
                    is_null: false,
                    stored_calculation: false,
                    boolean: Some(index != 0),
                    display_control: DisplayControl::Native(109),
                }
            );
        }
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}

#[test]
fn checkbox_unqualified_and_non_column_formats_keep_original_full_value_text() {
    for (format, control, scope) in [
        (Some(("Yes/No", 1, 10)), 106, 1),
        (Some(("Yes/No", 1, 10)), 111, 1),
        (Some(("Yes/No", 1, 10)), 999, 1),
        (Some(("Yes/No", 0, 10)), 109, 1),
        (Some(("Yes/No", 1, 3)), 109, 1),
        (Some(("Yes/No", 1, 10)), 109, 2),
        (Some(("0.00", 1, 10)), 109, 1),
        (None, 109, 1),
    ] {
        let bytes = source(format, control, scope);
        let (_directory, path, worker, _revision) = opened(&bytes);
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Items".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected unchanged raw Boolean text");
        };
        assert_eq!(rows[0][1].text, "false");
        assert_eq!(rows[1][1].text, "true");
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}
