//! Independent whole-image and generation/index preservation of actual sources.
use super::*;
use crate::{
    CreateColumn, DeclaredFieldAdditionDefinition, InsertFieldValue, OmittedRowInsert,
    UnadornedFieldAdditionDefinition,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{deletes, fields, typed_fields::Kind, unadorned_fields as raw},
    secondary_index::normal::{self, Population},
};

const SCOPE: CatalogScope = CatalogScope::new(60414);

fn budget() -> ResourceBudget {
    // This explicit consumer funds repeated complete 200k-row source/AFTER proofs.
    ResourceBudget::new(2_000_000_000, 20_000_000, 16_000_000_000)
}

fn image(before: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
        .unwrap()
}

fn request() -> UnadornedFieldAdditionDefinition {
    UnadornedFieldAdditionDefinition::capture(
        CreateColumn::new("Salary", ScalarType::Currency),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

fn last(before: &[u8], definition: u32) -> i32 {
    let at = definition as usize * 4096 + 20;
    i32::from_le_bytes(before[at..at + 4].try_into().unwrap())
}

#[test]
fn generated_history_survives_add_and_the_real_next_insert_uses_last_not_live_max() {
    let source = fields::source(fields::Case::FirstNumber);
    assert_eq!(source.roots.len(), 2);
    let removed = *source.roots.last().unwrap();
    let before = deletes::after(&source.bytes, source.definition, &[removed]);
    let historical_last = last(&before, source.definition);
    let old_rows = crate::read_table(
        &mut Cursor::new(&before),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(old_rows.rows().len(), 1);
    let Value::Int32(live_id) = &old_rows.rows()[0][0] else {
        panic!("actual generated I32 identity");
    };
    assert!(
        *live_id < historical_last,
        "fixture must retain generation history beyond live keys"
    );
    let expected = raw::after(&before, source.definition, "Salary", Kind::Currency, [0; 4]);
    let candidate = image(&before)
        .prepare_retained_unadorned_field_addition("Table1", request(), &mut budget(), &|| false)
        .unwrap();
    same(candidate.before_bytes(), &before);
    same(candidate.after_bytes(), &expected);
    assert_eq!(
        last(candidate.after_bytes(), source.definition),
        historical_last
    );
    assert!(matches!(
        image(&before).prepare_retained_declared_field_addition(
            "Table1",
            DeclaredFieldAdditionDefinition::capture(
                CreateColumn::new("Salary", ScalarType::Currency),
                &mut budget(),
                &|| false
            )
            .unwrap(),
            &mut budget(),
            &|| false
        ),
        Err(Error::Source)
    ));
    let expected_insert = raw::append_currency(&expected, source.definition, Some(0), None);
    let insert = image(candidate.after_bytes())
        .plan_recoverable_retained_omitted_row_inserts(
            "Table1",
            vec![OmittedRowInsert::new(vec![
                InsertFieldValue::Omitted,
                InsertFieldValue::Omitted,
                InsertFieldValue::Omitted,
            ])],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    same(insert.after_bytes(), &expected_insert);
    assert_eq!(
        last(insert.after_bytes(), source.definition),
        historical_last + 1
    );
    let observed = crate::read_table(
        &mut Cursor::new(insert.after_bytes()),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(observed.rows().len(), 2);
    assert_eq!(observed.rows()[1][0], Value::Int32(historical_last + 1));
    assert_eq!(observed.rows()[1][1], Value::Int32(0));
    assert_eq!(observed.rows()[1][2], Value::Null);
}

#[test]
fn existing_long_trees_keep_full_alias_membership_maps_and_counter_complements() {
    for descending in [false, true] {
        for population in [
            Population::Empty,
            Population::NullableDuplicate,
            Population::ThreeLeaves,
            Population::MultiLevel,
        ] {
            let source = normal::source(population, descending);
            let before = &source.expected; // The existing secondary is the independent AFTER of index creation.
            let expected = raw::after(
                before,
                source.target_definition,
                "Salary",
                Kind::Currency,
                [0; 4],
            );
            let candidate = image(before)
                .prepare_retained_unadorned_field_addition(
                    source.table,
                    request(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            same(candidate.after_bytes(), &expected);
            for page in &source.index_pages {
                let at = *page as usize * 4096;
                same(
                    &candidate.after_bytes()[at..at + 4096],
                    &before[at..at + 4096],
                );
            }
        }
        let companion = normal::protected_companion(descending);
        let expected = raw::after(
            &companion.expected,
            companion.target_definition,
            "Salary",
            Kind::Currency,
            [0; 4],
        );
        let candidate = image(&companion.expected)
            .prepare_retained_unadorned_field_addition(
                companion.table,
                request(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(candidate.after_bytes(), &expected);
        assert_eq!(
            last(candidate.after_bytes(), companion.target_definition),
            last(&companion.expected, companion.target_definition)
        );
    }
}

#[test]
fn malformed_indexes_and_generator_flags_are_not_ignored_by_metadata_only_add() {
    let source = normal::source(Population::ThreeLeaves, false);
    let mut wrong_owner = source.expected.to_vec();
    let owner_at = source.index_pages[0] as usize * 4096 + 4;
    wrong_owner[owner_at] ^= 1;
    let mut missing_member = source.expected.to_vec();
    let map_at = source.map_host as usize * 4096 + 4096 - 69 + 5;
    missing_member[map_at] &= !1;
    let generated = fields::source(fields::Case::FirstNumber);
    let mut wrong_generator = generated.bytes.to_vec();
    wrong_generator[generated.definition as usize * 4096 + 75 + 15] ^= 0x40;
    for (before, table) in [
        (&wrong_owner, source.table),
        (&missing_member, source.table),
        (&wrong_generator, "Table1"),
    ] {
        assert!(
            image(before)
                .prepare_retained_unadorned_field_addition(table, request(), &mut budget(), &|| {
                    false
                })
                .is_err()
        );
    }
    let native = image(&source.expected);
    assert!(matches!(
        native.prepare_retained_unadorned_field_addition(
            source.table,
            request(),
            &mut budget(),
            &|| true
        ),
        Err(Error::Inspect(crate::InspectError::Cancelled))
    ));
    let native = image(&source.expected);
    assert!(
        native
            .prepare_retained_unadorned_field_addition(
                source.table,
                request(),
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
}
