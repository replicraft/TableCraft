use super::*;
use tablecraft_core::values::{Currency, Value};

#[test]
fn typed_boolean_labels_preserve_null_and_cached_facts_without_parsing_text() {
    let mut output = Presentation::new(&|| false);
    for value in [
        Value::Boolean(false),
        Value::Boolean(true),
        Value::Null,
        Value::Text("true".into()),
    ] {
        let mut cell = output.scalar(&value, true).unwrap();
        if let CellKind::Scalar {
            display_control, ..
        } = &mut cell.kind
        {
            *display_control = DisplayControl::Native(TEXT_BOX);
        }
        let original_kind = cell.kind;
        let original_text = cell.text.clone();
        format_cell(&mut cell, "Yes", "No", &mut output).unwrap();
        assert_eq!(cell.kind, original_kind);
        assert_eq!(
            cell.text,
            match value {
                Value::Boolean(true) => "Yes".to_owned(),
                Value::Boolean(false) => "No".to_owned(),
                _ => original_text,
            }
        );
    }
}
use crate::{controls, source::Source};
use bytes::Bytes;
use tablecraft_ace::inspect_table_properties;
use tablecraft_testkit::fixtures::access::{field_presentation, wide_properties};

#[test]
fn typed_native_format_hints_require_actual_qualified_column_declarations() {
    for (format, flags, native_type, scope, expected) in [
        ("Short Date", 1, 10, 1, NativeFormat::ShortDate),
        ("Short Time", 1, 10, 1, NativeFormat::ShortTime),
        ("General Date", 1, 10, 1, NativeFormat::GeneralDate),
        (
            "€ #,##0.00;-€ #,##0.00",
            1,
            10,
            1,
            NativeFormat::EuroPrefixTwoDecimals,
        ),
        ("€ #,##0.000", 1, 10, 1, NativeFormat::Unsupported),
        ("Short Date", 0, 10, 1, NativeFormat::Unsupported),
        ("Short Date", 1, 3, 1, NativeFormat::Unsupported),
        ("Short Date", 1, 10, 2, NativeFormat::Unspecified),
    ] {
        let blocks = [
            wide_properties::names(&["Format"]),
            field_presentation::score_group(
                scope,
                &[field_presentation::property(
                    0,
                    flags,
                    native_type,
                    &wide_properties::text(format),
                )],
            ),
        ];
        let bytes = field_presentation::database(&blocks, true);
        let mut work = controls::budget();
        let (mut source, _) = Source::open(Bytes::from(bytes), "", &mut work, &|| false).unwrap();
        let properties =
            inspect_table_properties(&mut source, controls::SCOPE, "Items", &mut work, &|| false)
                .unwrap();
        let mut output = Presentation::new(&|| false);
        let mut rows = vec![vec![
            output
                .scalar(&Value::Currency(Currency::from_scaled(12_345)), false)
                .unwrap(),
        ]];
        let columns = vec![crate::column::plain("Score", &mut output).unwrap()];
        apply(Some(&properties), &columns, &mut rows, &mut output).unwrap();
        assert_eq!(
            rows[0][0].presentation,
            Some(ScalarPresentation::Currency {
                scaled: 12_345,
                native_format: expected,
            }),
            "{format}, flags={flags}, type={native_type}, scope={scope}"
        );
    }
}
