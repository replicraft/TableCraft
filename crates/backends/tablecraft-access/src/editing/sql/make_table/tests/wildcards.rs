//! Wildcard expansion borrows native metadata, never observed values or row width.
#![cfg(feature = "journal")]

use super::{SCOPE, budget, prepare};
use crate::editing::sql::NativeMakeTableError;
use std::io::Cursor;
use tablecraft_ace::{inspect_table_indexes, inspect_table_properties, inspect_table_schema};
use tablecraft_core::values::{Decimal, Value};
use tablecraft_query::{MakeTableError, QueryError};
use tablecraft_schema::SchemaError;
use tablecraft_testkit::fixtures::access::{
    canonical_initial_table_creation as initial,
    canonical_retained_add_table::{self as raw, fields, populated, rows},
};

fn seed() -> Vec<u8> {
    let mut seed = fields::source(fields::Case::FirstNumber).bytes;
    // Actual ROOT General2010, independent of the source Text field policy.
    for (index, mask) in [1, 27, 152, 253].into_iter().enumerate() {
        seed[110 + index] = [9, 4, 0, 1][index] ^ mask;
    }
    seed
}

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing byte"
    );
}

#[test]
fn bare_and_aliased_wildcards_keep_native_text_and_decimal_declarations_even_without_rows() {
    let seed = seed();
    let values = [
        (Some("é🦀"), Some(90_071_992_547_409_931_234)),
        (None, None),
    ];
    let before = populated::text_decimals(&seed, "Items", &values);
    for wildcard in ["*", "i.*", "I.*"] {
        for empty in [false, true] {
            let sql = format!(
                "SELECT {wildcard} INTO Archive FROM Items AS i{}",
                if empty { " WHERE False" } else { "" }
            );
            let staged = prepare(&before, &sql).unwrap();
            assert_eq!(staged.rows().len(), if empty { 0 } else { 2 });
            let expected =
                populated::text_decimals(&before, "Archive", if empty { &[] } else { &values });
            let plan = staged.finish(&mut budget(), &|| false).unwrap();
            same(plan.before_bytes(), &before);
            same(plan.after_bytes(), &expected);
            let schema = inspect_table_schema(
                &mut Cursor::new(plan.after_bytes()),
                SCOPE,
                "Archive",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(schema.columns()[0].name(), "Label");
            assert_eq!(schema.columns()[0].storage_length(), 14);
            assert_eq!(schema.columns()[1].name(), "Amount");
            assert_eq!(schema.columns()[1].decimal_parameters(), Some((28, 4)));
        }
    }
}

#[test]
fn wildcard_and_computed_projection_mapping_consumes_expanded_descriptors_in_written_order() {
    let seed = seed();
    let amount = 90_071_992_547_409_931_234;
    let before =
        populated::text_decimals(&seed, "Items", &[(Some("é🦀"), Some(amount)), (None, None)]);
    let expected = populated::computed::after(
        &before,
        "Archive",
        &[
            populated::computed::Row {
                label: Some("é🦀"),
                amount: Some(amount),
                score: Some(7),
            },
            populated::computed::Row {
                label: None,
                amount: None,
                score: Some(7),
            },
        ],
    );
    let staged = prepare(
        &before,
        "SELECT i.*,CLng(7) AS CopiedScore INTO Archive FROM Items AS i",
    )
    .unwrap();
    let plan = staged.finish(&mut budget(), &|| false).unwrap();
    same(plan.after_bytes(), &expected);

    let staged = prepare(
        &before,
        "SELECT CLng(7) AS CopiedScore,i.* INTO Archive FROM Items AS i",
    )
    .unwrap();
    assert_eq!(
        staged.rows(),
        &[
            vec![
                Value::Int32(7),
                Value::Text("é🦀".into()),
                Value::Decimal(Decimal::new(amount, 4).unwrap())
            ],
            vec![Value::Int32(7), Value::Null, Value::Null],
        ]
    );
    let plan = staged.finish(&mut budget(), &|| false).unwrap();
    let schema = inspect_table_schema(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Archive",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        schema
            .columns()
            .iter()
            .map(|field| field.name())
            .collect::<Vec<_>>(),
        ["CopiedScore", "Label", "Amount"]
    );
    assert_eq!(schema.columns()[1].storage_length(), 14);
    assert_eq!(schema.columns()[2].decimal_parameters(), Some((28, 4)));
}

#[test]
fn generated_primary_source_values_become_manual_destination_fields_without_properties() {
    let seed = seed();
    let definition = (seed.len() / 4096) as u32;
    let mut before = raw::desktop::append(&seed, "Items", raw::Target::GeneratedNumber);
    for score in [5, 9] {
        before = rows::append(
            &before,
            definition,
            rows::Profile::DesktopNumber,
            rows::Input::Number(score),
        );
    }
    let source_schema = inspect_table_schema(
        &mut Cursor::new(&before),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_ne!(source_schema.columns()[0].flags() & 4, 0);
    assert!(
        !inspect_table_properties(
            &mut Cursor::new(&before),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .groups()
        .is_empty()
    );

    for empty in [false, true] {
        let sql = format!(
            "SELECT i.* INTO Archive FROM Items AS i{}",
            if empty {
                " WHERE False"
            } else {
                " ORDER BY i.ID"
            }
        );
        let staged = prepare(&before, &sql).unwrap();
        assert!(staged.definition().is_manual());
        assert_eq!(staged.definition().primary_position(), None);
        let expected = populated::wildcards::id_scores(
            &before,
            "Archive",
            if empty {
                &[]
            } else {
                &[(1, Some(5)), (2, Some(9))]
            },
        );
        let plan = staged.finish(&mut budget(), &|| false).unwrap();
        same(plan.after_bytes(), &expected);
        let schema = inspect_table_schema(
            &mut Cursor::new(plan.after_bytes()),
            SCOPE,
            "Archive",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(schema.columns()[0].flags(), 3);
        assert_eq!(schema.columns()[1].flags(), 3);
        assert!(
            inspect_table_indexes(
                &mut Cursor::new(plan.after_bytes()),
                SCOPE,
                "Archive",
                &mut budget(),
                &|| false
            )
            .unwrap()
            .indexes()
            .is_empty()
        );
        assert!(
            inspect_table_properties(
                &mut Cursor::new(plan.after_bytes()),
                SCOPE,
                "Archive",
                &mut budget(),
                &|| false
            )
            .unwrap()
            .groups()
            .is_empty()
        );
    }
}

#[test]
fn wrong_wildcard_qualifiers_duplicates_and_unsupported_native_fields_are_not_silently_omitted() {
    let seed = seed();
    let definition = seed.len() / 4096;
    let mut corrupt = populated::numbers(&seed, "Items", &[Some(23)]);
    corrupt[(definition + 1) * 4096] = 255;
    for sql in [
        "SELECT other.* INTO Archive FROM Items AS i",
        "SELECT Items.* INTO Archive FROM Items AS i",
    ] {
        assert!(matches!(
            prepare(&corrupt, sql),
            Err(NativeMakeTableError::Shape)
        ));
    }
    assert!(matches!(
        prepare(&corrupt, "SELECT * INTO Archive FROM Items AS i"),
        Err(NativeMakeTableError::Query(_))
    ));

    let before = populated::text_decimals(&seed, "Items", &[(None, None)]);
    assert!(matches!(
        prepare(&before, "SELECT i.*,i.Label INTO Archive FROM Items AS i"),
        Err(NativeMakeTableError::Query(MakeTableError::Query(
            QueryError::Schema(SchemaError::Duplicate)
        )))
    ));
    let unsupported = initial::source(initial::Case::Mixed).bytes;
    assert!(matches!(
        prepare(
            &unsupported,
            "SELECT * INTO Archive FROM Table1 WHERE False"
        ),
        Err(NativeMakeTableError::Shape)
    ));
}
