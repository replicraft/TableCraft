//! Destination root defaults govern newly authored Text; source policies stay inert.
use crate::{
    AddTableError, CreateColumn, FirstTableDefinition, NativeImage, NativeSortOrder,
    PopulatedAddTableError, inspect_database_sort_order, inspect_table_schema,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    fields, populated as raw,
};

const SCOPE: CatalogScope = CatalogScope::new(19104);

fn budget() -> ResourceBudget {
    ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn root_order(bytes: &mut [u8], order: [u8; 4]) {
    // Independent ACE masked root descriptor, not MSysObjects.Name or a field.
    for (index, mask) in [1, 27, 152, 253].into_iter().enumerate() {
        bytes[110 + index] = order[index] ^ mask;
    }
}

fn source(order: [u8; 4], custom: [u8; 4]) -> Vec<u8> {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = seed.len() / 4096;
    let mut bytes = raw::text_decimals(&seed, "Original", &[(Some("source"), Some(12345))]);
    bytes[definition * 4096 + 63 + 11..definition * 4096 + 63 + 15].copy_from_slice(&custom);
    root_order(&mut bytes, order);
    bytes
}

fn definition() -> FirstTableDefinition {
    FirstTableDefinition::capture_manual_native(
        "Archive",
        &[
            CreateColumn::short_text("Label", 7).unwrap(),
            CreateColumn::decimal("Amount", DecimalConstraints::new(28, 4).unwrap()),
        ],
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing complete image byte"
    );
}

#[test]
fn native_empty_and_populated_text_use_legacy_or_2010_root_default_without_inheriting_source_custom_order()
 {
    for (root, custom) in [([9, 4, 0, 0], [9, 4, 0, 1]), ([9, 4, 0, 1], [9, 4, 0, 0])] {
        let before = source(root, custom);
        assert_eq!(
            inspect_database_sort_order(&mut Cursor::new(&before), &mut budget(), &|| false)
                .unwrap(),
            NativeSortOrder::from_native_bytes(root)
        );
        let old = inspect_table_schema(
            &mut Cursor::new(&before),
            SCOPE,
            "Original",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            old.columns()[0].text_sort_order(),
            Some(NativeSortOrder::from_native_bytes(custom))
        );
        for populated in [false, true] {
            let literals = if populated {
                vec![
                    (Some("a"), Some(12345)),
                    (None, None),
                    (Some(""), Some(-1234)),
                ]
            } else {
                vec![]
            };
            let mut expected = raw::text_decimals(&before, "Archive", &literals);
            // Literal new descriptor differs only by destination default encoding.
            let at = before.len() + 63 + 11;
            expected[at..at + 4].copy_from_slice(&root);
            let values = literals
                .iter()
                .map(|(text, coefficient)| {
                    vec![
                        text.map_or(Value::Null, |value| Value::Text(value.into())),
                        coefficient.map_or(Value::Null, |value| {
                            Value::Decimal(Decimal::new(value, 4).unwrap())
                        }),
                    ]
                })
                .collect();
            let after = if populated {
                image(&before)
                    .plan_populated_add_table(definition(), values, &mut budget(), &|| false)
                    .unwrap()
                    .after_bytes()
                    .to_vec()
            } else {
                image(&before)
                    .plan_add_table_native(definition(), &mut budget(), &|| false)
                    .unwrap()
                    .after_bytes()
                    .to_vec()
            };
            assert_image(&after, &expected);
            let new = inspect_table_schema(
                &mut Cursor::new(&after),
                SCOPE,
                "Archive",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                new.columns()[0].text_sort_order(),
                Some(NativeSortOrder::from_native_bytes(root))
            );
            let old = inspect_table_schema(
                &mut Cursor::new(&after),
                SCOPE,
                "Original",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                old.columns()[0].text_sort_order(),
                Some(NativeSortOrder::from_native_bytes(custom))
            );
        }
    }
}

#[test]
fn unsupported_root_defaults_refuse_only_new_text_not_nontext_native_construction() {
    for order in [
        [0, 0, 0, 0],
        [31, 4, 0, 1],
        [12, 4, 0, 0],
        [9, 4, 1, 1],
        [9, 4, 0, 2],
    ] {
        let before = source(order, [9, 4, 0, 1]);
        assert!(matches!(
            image(&before).plan_add_table_native(definition(), &mut budget(), &|| false),
            Err(AddTableError::Source)
        ));
        assert!(matches!(
            image(&before).plan_populated_add_table(definition(), vec![], &mut budget(), &|| false),
            Err(PopulatedAddTableError::Append(AddTableError::Source))
        ));
        let manual = FirstTableDefinition::capture_manual_native(
            "Numbers",
            &[CreateColumn::new("CopiedScore", ScalarType::Int32)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let expected = raw::numbers(&before, "Numbers", &[Some(7), None]);
        let plan = image(&before)
            .plan_populated_add_table(
                manual,
                vec![vec![Value::Int32(7)], vec![Value::Null]],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_image(plan.after_bytes(), &expected);
    }
}
