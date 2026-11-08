//! Public SDK Number declarations, retained Required presence and producer pins.
//! Connected as a child of the existing tests module to reuse local type glue.
use super::{budget, hash, image};
use crate::{
    CreateColumn, CreatedDefaultZeroColumnRequired, PropertyGroupKind, create_ace16_database,
    inspect_table_properties,
};
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_created_number as raw;

fn required(bytes: &[u8], table: &str, column: &str) -> Option<bool> {
    let image = image(bytes);
    let properties = inspect_table_properties(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    properties
        .group(PropertyGroupKind::Column, column)
        .and_then(|group| {
            group
                .properties()
                .iter()
                .find(|property| property.name() == "Required")
        })
        .map(|property| {
            assert_eq!(property.native_type(), 1);
            assert_eq!(property.flags(), 1);
            assert_eq!(property.raw_value(), [0]);
            property.boolean().unwrap()
        })
}

fn qualify(case: raw::Source, table: &str, column: &str, before_hash: &str, after_hash: &str) {
    let before = raw::source(case);
    let untouched = before.clone();
    let after = raw::append(&before, case, column);
    assert_eq!(hash(&before), before_hash);
    assert_eq!(hash(&after), after_hash);
    assert_eq!((before.len(), after.len()), (40960, 45056));
    let old_required = if matches!(case, raw::Source::GuiRequiredZero) {
        Some(false)
    } else {
        None
    };
    if !matches!(case, raw::Source::IdOnly) {
        assert_eq!(required(&before, table, "Score"), old_required);
        assert_eq!(required(&after, table, "Score"), old_required);
    }
    assert_eq!(required(&after, table, column), Some(false));
    let candidate = image(&before)
        .prepare_created_add_default_zero_int32_column_candidate(
            table,
            column,
            CreatedDefaultZeroColumnRequired::StoredFalse,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(before, untouched);
    assert_eq!(candidate.before_bytes(), before);
    assert_eq!(candidate.after_bytes(), after);
    assert_eq!(
        candidate.required_property(),
        CreatedDefaultZeroColumnRequired::StoredFalse
    );
    assert_eq!(
        required(candidate.after_bytes(), table, column),
        Some(false)
    );
}

#[test]
fn id_only_number_uses_sdk_default_then_required_dictionary_and_complete_after() {
    qualify(
        raw::Source::IdOnly,
        "Table1",
        "Score",
        "b52d124892cdaab8fbe9c5028c876f57f1f82ee280cea7984aad68f1ead5eb13",
        "a6a9e18cd35dc2bf018e5cd554a68b5b60d56023fd78a4a0ecba627ab9041d72",
    );
}

#[test]
fn existing_gui_number_preserves_required_then_default_dictionary_and_complete_after() {
    qualify(
        raw::Source::GuiRequiredZero,
        "Table1",
        "Extra",
        "9bb835b86e8e0789728834ae8871ab9b51d647c69bea89a091d15fd14cd8a90e",
        "b314c02b5f67c8deebb564f7e4deae5cd5a5e28e1f67f3122b1cb8d14507c3a4",
    );
}

#[test]
fn existing_default_only_keeps_old_required_absent_and_adds_exact_new_false() {
    qualify(
        raw::Source::DefaultOnly,
        "Items",
        "Extra",
        "6a17e6cf803d42e10a5bc7f240852ae44c69fd2b10d451e75379d9166aacd808",
        "05dcae27ffd63cba552e47740c60649d46bb265730b6b202a384e69ebb249072",
    );
}

#[test]
fn independent_gui_policy_source_equals_actual_constructive_creator_bytes() {
    let zero = Value::Int32(0);
    let columns = [
        CreateColumn::new("ID", ScalarType::Int32)
            .with_primary_key("PrimaryKey")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("Score", ScalarType::Int32)
            .with_required(false)
            .with_default_literal(&zero)
            .unwrap(),
    ];
    let actual = create_ace16_database("Table1", &columns, &[], &mut budget(), &|| false).unwrap();
    let raw = raw::source(raw::Source::GuiRequiredZero);
    assert_eq!(actual.as_bytes(), raw);
    assert_eq!(
        hash(actual.as_bytes()),
        "9bb835b86e8e0789728834ae8871ab9b51d647c69bea89a091d15fd14cd8a90e"
    );
}

#[test]
fn canonical_existing_required_true_refuses_instead_of_becoming_nullable_false() {
    let mut before = raw::source(raw::Source::GuiRequiredZero);
    let entry = [9, 0, 1, 1, 0, 0, 1, 0, 0];
    let positions = before
        .windows(entry.len())
        .enumerate()
        .filter_map(|(position, bytes)| (bytes == entry).then_some(position))
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 1);
    before[positions[0] + entry.len() - 1] = 255;
    // The broader existing constructor proof accepts this exact canonical
    // Required=true source; the new Number profile must reject its policy.
    image(&before)
        .prepare_created_add_int32_column_candidate("Table1", "Other", &mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        image(&before).prepare_created_add_default_zero_int32_column_candidate(
            "Table1",
            "Extra",
            CreatedDefaultZeroColumnRequired::StoredFalse,
            &mut budget(),
            &|| false,
        ),
        Err(crate::CreatedDefaultZeroAddColumnError::Source)
    ));
    assert_eq!(before[positions[0] + entry.len() - 1], 255);
}
