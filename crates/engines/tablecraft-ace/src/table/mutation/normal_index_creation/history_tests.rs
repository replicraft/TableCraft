//! New identity policy preserves the original producer and every existing identity.
use crate::{
    CreateColumn, EditError, NativeImage, SecondaryIndexCreate, UnadornedFieldAdditionDefinition,
    WriteBlocker,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::secondary_index::normal::{
    self as raw, Population, dropping, history,
};

const SCOPE: CatalogScope = CatalogScope::new(28612);
fn budget(rows: usize) -> ResourceBudget {
    ResourceBudget::new(
        2 * 1024 * 1024 * 1024,
        rows.max(1) as u64 * 100 + 100_000,
        rows.max(1) as u64 * 80_000 + 32_000_000,
    )
}
fn image(bytes: &[u8], budget: &mut ResourceBudget) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, budget, &|| false).unwrap()
}
fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing complete image byte"
    );
}
#[test]
fn fresh_creation_after_alias_drop_keeps_original_ids_and_matches_independent_new_identity() {
    for descending in [false, true] {
        let case = history::source_with_direction(descending);
        let request = SecondaryIndexCreate::new(case.index_name, case.column, descending);
        let mut work = budget(case.rows.len());
        assert!(matches!(
            image(&case.before, &mut work).prepare_retained_long_index_creation(
                case.table,
                &request,
                &mut work,
                &|| false
            ),
            Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        ));
        let candidate = image(&case.before, &mut work)
            .prepare_retained_fresh_long_index_creation(case.table, &request, &mut work, &|| false)
            .unwrap();
        same(candidate.before_bytes(), &case.before);
        same(candidate.candidate_bytes(), &case.expected);
        assert!(candidate.is_fresh_construction());
        assert_eq!(candidate.logical_index_number(), 3);
        assert_eq!(candidate.physical_index_number(), 2);
        assert_eq!(candidate.native_number(), 4);
    }
}

#[test]
fn actual_alias_drop_then_unadorned_add_then_fresh_creation_preserves_the_complete_history() {
    for descending in [false, true] {
        let original = raw::protected_companion(false);
        let alias = dropping::surviving_alias(&original, "ScoreAlias");
        let expected = history::source_with_direction(descending);
        let mut work = budget(original.rows.len());

        let dropped = image(&alias.before, &mut work)
            .prepare_retained_index_drop(original.table, original.index_name, &mut work, &|| false)
            .unwrap();
        same(dropped.before_bytes(), &alias.before);
        same(dropped.candidate_bytes(), &alias.expected);
        assert_eq!(dropped.logical_index_number(), 1);
        assert_eq!(dropped.physical_index_number(), 1);
        assert_eq!(dropped.retired_pages(), 0);

        let definition = UnadornedFieldAdditionDefinition::capture(
            CreateColumn::new("OtherScore", ScalarType::Int32),
            &mut work,
            &|| false,
        )
        .unwrap();
        let added = image(dropped.candidate_bytes(), &mut work)
            .prepare_retained_unadorned_field_addition(
                original.table,
                definition,
                &mut work,
                &|| false,
            )
            .unwrap();
        same(added.before_bytes(), &alias.expected);
        same(added.after_bytes(), &expected.before);
        assert_eq!(added.native_number(), 4);
        assert_eq!(added.column_id(), 4);
        assert_eq!(added.column_count(), 5);

        let created = image(added.after_bytes(), &mut work)
            .prepare_retained_fresh_long_index_creation(
                original.table,
                &SecondaryIndexCreate::new("OtherLookup", "OtherScore", descending),
                &mut work,
                &|| false,
            )
            .unwrap();
        same(created.before_bytes(), &expected.before);
        same(created.candidate_bytes(), &expected.expected);
        assert_eq!(created.logical_index_number(), 3);
        assert_eq!(created.physical_index_number(), 2);
        assert_eq!(created.native_number(), 4);
        assert_eq!(created.indexed_rows(), original.rows.len());

        let checked = crate::check_table(
            &mut Cursor::new(created.candidate_bytes()),
            SCOPE,
            original.table,
            &mut work,
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), original.rows.len());
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == crate::IndexCheckStatus::Checked)
        );
    }
}

#[test]
fn fresh_policy_retains_count_based_bytes_when_the_original_identity_is_available() {
    for population in [
        Population::Empty,
        Population::NullableDuplicate,
        Population::ThreeLeaves,
        Population::MultiLevel,
    ] {
        for descending in [false, true] {
            let case = raw::source(population, descending);
            let mut work = budget(case.rows.len());
            let candidate = image(&case.before, &mut work)
                .prepare_retained_fresh_long_index_creation(
                    case.table,
                    &SecondaryIndexCreate::new(case.index_name, case.column, descending),
                    &mut work,
                    &|| false,
                )
                .unwrap();
            same(candidate.candidate_bytes(), &case.expected);
            assert_eq!(candidate.logical_index_number(), 0);
            assert!(candidate.is_fresh_construction());
        }
    }
}
#[test]
fn existing_indexed_fields_cancellation_and_zero_budget_still_refuse_fresh_candidates() {
    let case = history::source();
    let request = SecondaryIndexCreate::new("DifferentLookup", case.column, false);
    let mut work = budget(case.rows.len());
    assert!(matches!(
        image(&case.expected, &mut work).prepare_retained_fresh_long_index_creation(
            case.table,
            &request,
            &mut work,
            &|| false,
        ),
        Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
    ));
    assert!(
        image(&case.before, &mut work)
            .prepare_retained_fresh_long_index_creation(case.table, &request, &mut work, &|| true,)
            .is_err()
    );
    assert!(
        image(&case.before, &mut work)
            .prepare_retained_fresh_long_index_creation(
                case.table,
                &request,
                &mut ResourceBudget::new(0, 0, 0),
                &|| false,
            )
            .is_err()
    );
}

#[test]
fn fresh_identity_policy_checks_native_u32_exhaustion_and_work_without_a_smaller_ceiling() {
    let case = history::source();
    let mut work = budget(case.rows.len());
    let schema = crate::inspect_table_schema(
        &mut Cursor::new(&case.before),
        SCOPE,
        case.table,
        &mut work,
        &|| false,
    )
    .unwrap();
    let logical = case.target_definition as usize * 4096
        + schema.layout.index_start
        + schema.layout.physical_indexes * 52;
    let mut exhausted = case.before.to_vec();
    // Passive metadata represents the full native u32 word. Source authority
    // remains separate; the allocator must not wrap on any such metadata.
    exhausted[logical + 4..logical + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    let metadata = crate::inspect_table_indexes(
        &mut Cursor::new(&exhausted),
        SCOPE,
        case.table,
        &mut work,
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        super::source::fresh_logical(2, metadata.indexes(), &mut work, &|| false),
        Err(crate::InspectError::StructureLimit)
    ));
    assert_eq!(
        super::source::fresh_logical(1, metadata.indexes(), &mut work, &|| false).unwrap(),
        1
    );
    assert!(super::source::fresh_logical(2, metadata.indexes(), &mut work, &|| true).is_err());
    assert!(
        super::source::fresh_logical(
            2,
            metadata.indexes(),
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn fresh_namespace_refuses_unqualified_unicode_without_changing_original_name_admission() {
    let plain = raw::source(Population::NullableDuplicate, false);
    let unicode_request = SecondaryIndexCreate::new("ſcoreLookup", plain.column, false);
    let mut work = budget(plain.rows.len());
    let old = image(&plain.before, &mut work)
        .prepare_retained_long_index_creation(plain.table, &unicode_request, &mut work, &|| false)
        .unwrap();
    assert_eq!(old.index_name(), "ſcoreLookup");
    assert!(matches!(
        image(&plain.before, &mut work).prepare_retained_fresh_long_index_creation(
            plain.table,
            &unicode_request,
            &mut work,
            &|| false,
        ),
        Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    ));

    let case = history::source();
    let schema = crate::inspect_table_schema(
        &mut Cursor::new(&case.before),
        SCOPE,
        case.table,
        &mut work,
        &|| false,
    )
    .unwrap();
    let base = case.target_definition as usize * 4096;
    let mut at = base
        + schema.layout.index_start
        + schema.layout.physical_indexes * 52
        + schema.layout.logical_indexes * 28;
    let mut unqualified = case.before.to_vec();
    let mut replaced = false;
    for _ in 0..schema.layout.logical_indexes {
        let count = usize::from(u16::from_le_bytes(
            unqualified[at..at + 2].try_into().unwrap(),
        ));
        let units = unqualified[at + 2..at + 2 + count]
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(u16::from_le_bytes)
            .collect::<Vec<_>>();
        if String::from_utf16(&units).unwrap() == "ScoreAlias" {
            unqualified[at + 2..at + 4].copy_from_slice(&0x017Fu16.to_le_bytes());
            replaced = true;
        }
        at += 2 + count;
    }
    assert!(replaced);
    let read = crate::inspect_table_indexes(
        &mut Cursor::new(&unqualified),
        SCOPE,
        case.table,
        &mut work,
        &|| false,
    )
    .unwrap();
    assert!(
        read.indexes()
            .iter()
            .any(|index| index.name() == "ſcoreAlias")
    );
    let checked = crate::check_table(
        &mut Cursor::new(&unqualified),
        SCOPE,
        case.table,
        &mut work,
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), case.rows.len());
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == crate::IndexCheckStatus::Checked)
    );
    assert!(matches!(
        image(&unqualified, &mut work).prepare_retained_fresh_long_index_creation(
            case.table,
            &SecondaryIndexCreate::new(case.index_name, case.column, false),
            &mut work,
            &|| false,
        ),
        Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    ));
}
