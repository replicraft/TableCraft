//! Exact independent scalar declarations preserve old rows and absent policies.
use super::*;
use crate::{CreateColumn, inspect_table_properties};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_schema::DecimalConstraints;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    declared_fields, fields,
    typed_fields::{KINDS, Kind},
    unadorned_fields as raw,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}
fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(30600),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}
fn column(kind: Kind) -> CreateColumn<'static> {
    match kind {
        Kind::Boolean => CreateColumn::new("AddedValue", ScalarType::Boolean),
        Kind::Byte => CreateColumn::new("AddedValue", ScalarType::Byte),
        Kind::Int16 => CreateColumn::new("AddedValue", ScalarType::Int16),
        Kind::Int32 => CreateColumn::new("AddedValue", ScalarType::Int32),
        Kind::Int64 => CreateColumn::new("AddedValue", ScalarType::Int64),
        Kind::Single => CreateColumn::new("AddedValue", ScalarType::Single),
        Kind::Double => CreateColumn::new("AddedValue", ScalarType::Double),
        Kind::Currency => CreateColumn::new("AddedValue", ScalarType::Currency),
        Kind::DateTime => CreateColumn::new("AddedValue", ScalarType::DateTime),
        Kind::Guid => CreateColumn::new("AddedValue", ScalarType::Guid),
        Kind::ShortText { units, .. } => {
            CreateColumn::short_text("AddedValue", usize::from(units)).unwrap()
        }
        Kind::Decimal { precision, scale } => CreateColumn::decimal(
            "AddedValue",
            DecimalConstraints::new(precision, scale).unwrap(),
        ),
    }
}
fn request(kind: Kind) -> UnadornedFieldAdditionDefinition {
    UnadornedFieldAdditionDefinition::capture(column(kind), &mut budget(), &|| false).unwrap()
}
fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing complete image byte"
    );
}
fn root_order(bytes: &mut [u8], order: [u8; 4]) {
    for (index, mask) in [1, 27, 152, 253].into_iter().enumerate() {
        bytes[110 + index] = order[index] ^ mask;
    }
}

#[test]
fn all_twelve_unadorned_kinds_preserve_complete_keyless_and_generated_sources() {
    let plain = declared_fields::source();
    let generated = fields::source(fields::Case::FirstNumber);
    for (before, definition, table) in [
        (&plain.bytes, plain.definition, "Items"),
        (&generated.bytes, generated.definition, "Table1"),
    ] {
        for original in KINDS {
            let kind = match original {
                Kind::ShortText { units, .. } => Kind::ShortText {
                    units,
                    compression: false,
                },
                other => other,
            };
            let mut before = before.to_vec();
            root_order(&mut before, [9, 4, 0, 1]);
            let expected = raw::after(&before, definition, "AddedValue", kind, [9, 4, 0, 1]);
            let candidate = image(&before)
                .prepare_retained_unadorned_field_addition(
                    table,
                    request(kind),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            same(candidate.before_bytes(), &before);
            same(candidate.after_bytes(), &expected);
            assert_eq!(candidate.definition_page(), definition);
            let captured = candidate.definition().column();
            assert_eq!(captured.required(), None);
            assert_eq!(captured.allow_zero_length(), None);
            assert!(captured.default_literal().is_none());
            assert_eq!(captured.description(), None);
            let props = inspect_table_properties(
                &mut Cursor::new(&expected),
                CatalogScope::new(30601),
                table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(
                props
                    .groups()
                    .iter()
                    .all(|group| !group.name().eq_ignore_ascii_case("AddedValue"))
            );
            let rows = crate::read_table(
                &mut Cursor::new(&expected),
                CatalogScope::new(30602),
                table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let missing = if matches!(kind, Kind::Boolean) {
                Value::Boolean(false)
            } else {
                Value::Null
            };
            assert!(!rows.rows().is_empty());
            assert!(rows.rows().iter().all(|row| row.last() == Some(&missing)));
        }
    }
}

#[test]
fn text_uses_actual_root_policy_and_decimal_retains_native_parameters() {
    let source = declared_fields::source();
    for order in [[9, 4, 0, 0], [9, 4, 0, 1]] {
        let mut before = source.bytes.to_vec();
        root_order(&mut before, order);
        for kind in [
            Kind::ShortText {
                units: 1,
                compression: false,
            },
            Kind::ShortText {
                units: 255,
                compression: false,
            },
            Kind::Decimal {
                precision: 1,
                scale: 0,
            },
            Kind::Decimal {
                precision: 28,
                scale: 28,
            },
        ] {
            let expected = raw::after(&before, source.definition, "AddedValue", kind, order);
            let candidate = image(&before)
                .prepare_retained_unadorned_field_addition(
                    "Items",
                    request(kind),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            same(candidate.after_bytes(), &expected);
            let captured = candidate.definition().column();
            match kind {
                Kind::ShortText { units, .. } => {
                    assert_eq!(captured.max_text_utf16_units(), Some(usize::from(units)))
                }
                Kind::Decimal { precision, scale } => assert_eq!(
                    captured.decimal_constraints(),
                    Some(DecimalConstraints::new(precision, scale).unwrap())
                ),
                _ => unreachable!(),
            }
        }
    }
    let mut unsupported = source.bytes.to_vec();
    root_order(&mut unsupported, [12, 4, 0, 0]);
    assert!(
        image(&unsupported)
            .prepare_retained_unadorned_field_addition(
                "Items",
                request(Kind::ShortText {
                    units: 9,
                    compression: false
                }),
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    let currency = image(&unsupported)
        .prepare_retained_unadorned_field_addition(
            "Items",
            request(Kind::Currency),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    same(
        currency.after_bytes(),
        &raw::after(
            &unsupported,
            source.definition,
            "AddedValue",
            Kind::Currency,
            [0; 4],
        ),
    );
}

#[test]
fn annotated_unsupported_requests_and_forged_effects_never_gain_authority() {
    for column in [
        CreateColumn::new("AddedValue", ScalarType::Currency).with_required(false),
        CreateColumn::short_text("AddedValue", 5)
            .unwrap()
            .with_unicode_compression(true)
            .unwrap(),
        CreateColumn::short_binary("AddedValue", 5).unwrap(),
        CreateColumn::new("AddedValue", ScalarType::Text),
    ] {
        assert!(
            UnadornedFieldAdditionDefinition::capture(column, &mut budget(), &|| false).is_err()
        );
    }
    let source = declared_fields::source();
    let request = request(Kind::Decimal {
        precision: 28,
        scale: 4,
    });
    let captured = unadorned_source::capture(
        image(&source.bytes),
        "Items",
        request.name(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let layout = Plan::for_field(
        captured.proof().image(),
        &captured.proof().table().schema,
        request.field(0).unwrap(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let expected = raw::after(
        &source.bytes,
        source.definition,
        "AddedValue",
        Kind::Decimal {
            precision: 28,
            scale: 4,
        },
        [0; 4],
    );
    for offset in [
        3 * 4096 + 3000,
        source.definition as usize * 4096 + 88 + 11,
        source.definition as usize * 4096 + 43,
    ] {
        let mut forged = expected.to_vec();
        forged[offset] ^= 1;
        assert!(
            proof::candidate(
                &captured,
                &layout,
                &request,
                &forged,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    assert!(
        image(&source.bytes)
            .prepare_retained_unadorned_field_addition("Items", request, &mut budget(), &|| true)
            .is_err()
    );
}
