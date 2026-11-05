use super::Presentation;
use crate::{Cell, CellKind, Column, Error, HorizontalAlignment};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Decimal, Double, ExtendedDateTime, LegacyDateTime, Single, Value},
};

#[test]
fn column_envelopes_and_header_payloads_share_the_bounded_output_pool() {
    let envelope = 2 * size_of::<Column>() as u64;
    let mut exact = Presentation {
        work: ResourceBudget::new(envelope + 3, 0, 10),
        cancelled: &|| false,
    };
    let mut columns = exact.columns(2).unwrap();
    columns.push(crate::column::plain("Id", &mut exact).unwrap());
    columns.push(crate::column::plain("X", &mut exact).unwrap());
    assert_eq!(
        columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["Id", "X"]
    );
    assert_eq!(exact.work.remaining(Resource::Bytes), 0);

    let mut missing_envelope_byte = Presentation {
        work: ResourceBudget::new(envelope - 1, 0, 10),
        cancelled: &|| false,
    };
    assert!(
        matches!(missing_envelope_byte.columns(2), Err(Error::Limit(limit))
        if limit.resource == Resource::Bytes && limit.remaining == envelope - 1)
    );
    assert_eq!(
        missing_envelope_byte.work.remaining(Resource::Bytes),
        envelope - 1
    );

    let mut missing_payload_byte = Presentation {
        work: ResourceBudget::new(envelope + 2, 0, 10),
        cancelled: &|| false,
    };
    let mut columns = missing_payload_byte.columns(2).unwrap();
    columns.push(crate::column::plain("Id", &mut missing_payload_byte).unwrap());
    assert!(
        matches!(crate::column::plain("X", &mut missing_payload_byte), Err(Error::Limit(limit))
        if limit.resource == Resource::Bytes && limit.remaining == 0)
    );
    assert_eq!(
        columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["Id"]
    );
}

#[test]
fn boolean_facts_are_independent_of_cached_provenance_and_expression_text() {
    let mut output = Presentation::new(&|| false);
    for (value, boolean, is_null) in [
        (Value::Boolean(true), Some(true), false),
        (Value::Boolean(false), Some(false), false),
        (Value::Null, None, true),
        (Value::Text("true".into()), None, false),
        (Value::Int32(-1), None, false),
    ] {
        let cell = output.scalar(&value, true).unwrap();
        assert_eq!(
            cell.kind,
            CellKind::Scalar {
                is_null,
                stored_calculation: true,
                boolean,
                display_control: crate::DisplayControl::Uninspected,
            }
        );
    }
    let expression = Cell::expression("true".into());
    assert_eq!(
        expression.kind,
        CellKind::Scalar {
            is_null: false,
            stored_calculation: false,
            boolean: None,
            display_control: crate::DisplayControl::Uninspected
        }
    );
}

#[test]
fn alignment_uses_typed_carriers_without_changing_text_or_provenance() {
    let mut output = Presentation::new(&|| false);
    for value in [
        Value::Byte(12),
        Value::Int16(-12),
        Value::Int32(12),
        Value::Int64(12),
        Value::Single(Single::from_float(12.0)),
        Value::Double(Double::from_float(12.0)),
        Value::Currency(Currency::from_scaled(120_000)),
        Value::Decimal(Decimal::new(120, 1).unwrap()),
        Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
        Value::DateTimeExtended(ExtendedDateTime::new(2026, 5, 12, 9, 30, 0, 0).unwrap()),
    ] {
        let expected = output.value(&value).unwrap();
        let cell = output.scalar(&value, true).unwrap();
        assert_eq!(cell.alignment, HorizontalAlignment::Right);
        assert_eq!(cell.text, expected);
        assert!(matches!(
            cell.kind,
            CellKind::Scalar {
                is_null: false,
                stored_calculation: true,
                ..
            }
        ));
    }
    for value in [
        Value::Text("123.00".into()),
        Value::Text("2026-05-12".into()),
        Value::Null,
        Value::Boolean(true),
    ] {
        let cell = output.scalar(&value, false).unwrap();
        assert_eq!(cell.alignment, HorizontalAlignment::Left);
        assert_eq!(cell.text, output.value(&value).unwrap());
        assert!(
            matches!(cell.kind, CellKind::Scalar { is_null, .. } if is_null == matches!(value, Value::Null))
        );
    }
}

#[test]
fn typed_display_facts_preserve_exact_currency_date_bits_and_extended_ticks() {
    use crate::{DateRepresentation, NativeFormat, ScalarPresentation};
    let mut output = Presentation::new(&|| false);
    for scaled in [i64::MIN, -1, 0, 1, i64::MAX] {
        let cell = output
            .scalar(&Value::Currency(Currency::from_scaled(scaled)), false)
            .unwrap();
        assert_eq!(
            cell.presentation,
            Some(ScalarPresentation::Currency {
                scaled,
                native_format: NativeFormat::Unspecified,
            })
        );
    }

    let legacy = LegacyDateTime::from_ole_days(-0.5).unwrap();
    let cell = output.scalar(&Value::DateTime(legacy), false).unwrap();
    assert_eq!(
        cell.presentation,
        Some(ScalarPresentation::DateTime {
            calendar: jiff::civil::date(1899, 12, 30).at(12, 0, 0, 0),
            representation: DateRepresentation::LegacyBits((-0.5_f64).to_bits()),
            native_format: NativeFormat::Unspecified,
        })
    );
    let extended = ExtendedDateTime::new(2026, 4, 12, 15, 4, 5, 1_234_567).unwrap();
    let cell = output
        .scalar(&Value::DateTimeExtended(extended), false)
        .unwrap();
    assert_eq!(
        cell.presentation,
        Some(ScalarPresentation::DateTime {
            calendar: jiff::civil::date(2026, 4, 12).at(15, 4, 5, 123_456_700),
            representation: DateRepresentation::Extended,
            native_format: NativeFormat::Unspecified,
        })
    );
    assert!(
        output
            .scalar(&Value::Text("12.34".into()), false)
            .unwrap()
            .presentation
            .is_none()
    );
    assert!(
        output
            .scalar(&Value::Null, false)
            .unwrap()
            .presentation
            .is_none()
    );
    assert!(Cell::expression("12.34".into()).presentation.is_none());
}
