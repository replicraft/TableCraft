//! Raw inverse images qualify complete DROP effects, not only metadata deletion.
use super::proof;
use crate::{EditError, NativeImage, WriteBlocker};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::secondary_index::normal::{
    self as raw, Population, dropping,
};

const SCOPE: CatalogScope = CatalogScope::new(28610);

fn allowance(rows: usize) -> ResourceBudget {
    // Actual complete membership, fresh source recapture and full-image proof
    // share one explicit consumer allowance, not a production population ceiling.
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
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing complete image byte"
    );
}

#[test]
fn nullable_directional_multilevel_drop_reclaims_the_complete_tree_and_dedicated_map() {
    for population in [
        Population::Empty,
        Population::NullableDuplicate,
        Population::ThreeLeaves,
        Population::MultiLevel,
    ] {
        for descending in [false, true] {
            let case = raw::source(population, descending);
            let expected = dropping::after(&case);
            let mut budget = allowance(case.rows.len());
            let plan = image(&case.expected, &mut budget)
                .prepare_retained_index_drop(case.table, case.index_name, &mut budget, &|| false)
                .unwrap_or_else(|error| {
                    panic!(
                        "rows={} descending={descending}: {error:?}",
                        case.rows.len()
                    )
                });
            same(plan.before_bytes(), &case.expected);
            same(plan.candidate_bytes(), &expected);
            assert_eq!(plan.table(), case.table);
            assert_eq!(plan.index_name(), case.index_name);
            assert_eq!(plan.definition_page(), case.target_definition);
            assert_eq!(plan.logical_index_number(), 0);
            assert_eq!(plan.physical_index_number(), 0);
            assert_eq!(plan.retired_pages(), case.index_pages.len() + 1);
            if matches!(population, Population::MultiLevel) {
                assert!(case.levels >= 3);
            }
        }
    }
}

#[test]
fn generated_primary_currency_unicode_and_original_data_survive_secondary_drop() {
    for descending in [false, true] {
        let case = raw::protected_companion(descending);
        let expected = dropping::after(&case);
        let mut budget = allowance(case.rows.len());
        let plan = image(&case.expected, &mut budget)
            .prepare_retained_index_drop(case.table, case.index_name, &mut budget, &|| false)
            .unwrap();
        same(plan.candidate_bytes(), &expected);
        assert_eq!(plan.logical_index_number(), 1);
        assert_eq!(plan.physical_index_number(), 1);
    }
}

#[test]
fn primary_missing_corrupt_owner_cancellation_and_exhausted_work_do_not_return_candidates() {
    let case = raw::protected_companion(false);
    let mut budget = allowance(case.rows.len());
    let inspected = crate::inspect_table_indexes(
        &mut Cursor::new(&case.expected),
        SCOPE,
        case.table,
        &mut budget,
        &|| false,
    )
    .unwrap();
    let primary = inspected
        .indexes()
        .iter()
        .find(|index| index.data_number() == 0)
        .unwrap();
    assert!(matches!(
        image(&case.expected, &mut budget).prepare_retained_index_drop(
            case.table,
            primary.name(),
            &mut budget,
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
    ));
    assert!(
        image(&case.expected, &mut budget)
            .prepare_retained_index_drop(case.table, "Missing", &mut budget, &|| false)
            .is_err()
    );
    assert!(
        image(&case.expected, &mut budget)
            .prepare_retained_index_drop(case.table, case.index_name, &mut budget, &|| true)
            .is_err()
    );
    assert!(
        image(&case.expected, &mut budget)
            .prepare_retained_index_drop(
                case.table,
                case.index_name,
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
    for page in [case.root, case.map_host] {
        let mut malformed = case.expected.to_vec();
        let owner = page as usize * 4096 + 4;
        malformed[owner..owner + 4].copy_from_slice(&999u32.to_le_bytes());
        assert!(
            image(&malformed, &mut budget)
                .prepare_retained_index_drop(case.table, case.index_name, &mut budget, &|| false)
                .is_err(),
            "wrong tree/map owner on page {page}"
        );
    }
}

#[test]
fn independent_proof_rejects_unretired_roots_and_protected_header_mutations() {
    let case = raw::source(Population::ThreeLeaves, false);
    let expected = dropping::after(&case);
    for offset in [300, case.root as usize * 4096 + 100] {
        let mut forged = expected.to_vec();
        forged[offset] ^= 1;
        let mut budget = allowance(case.rows.len());
        let before = image(&case.expected, &mut budget);
        let after = image(&forged, &mut budget);
        assert!(
            proof::prove(
                &before,
                &after,
                case.table,
                case.index_name,
                &mut budget,
                &|| false
            )
            .is_err(),
            "forged offset {offset}"
        );
    }
}

#[test]
fn surviving_logical_alias_keeps_its_original_identity_and_entire_physical_footprint() {
    let case = raw::protected_companion(false);
    let alias = dropping::surviving_alias(&case, "ScoreAlias");
    let mut budget = allowance(case.rows.len());
    let plan = image(&alias.before, &mut budget)
        .prepare_retained_index_drop(case.table, case.index_name, &mut budget, &|| false)
        .unwrap();
    same(plan.candidate_bytes(), &alias.expected);
    assert_eq!(plan.retired_pages(), 0);
    for page in case.index_pages.iter().copied().chain([case.map_host]) {
        let start = page as usize * 4096;
        same(
            &alias.before[start..start + 4096],
            &plan.candidate_bytes()[start..start + 4096],
        );
    }
    let observed = crate::inspect_table_indexes(
        &mut Cursor::new(plan.candidate_bytes()),
        SCOPE,
        case.table,
        &mut budget,
        &|| false,
    )
    .unwrap();
    let surviving = observed
        .indexes()
        .iter()
        .find(|index| index.name() == "ScoreAlias")
        .unwrap();
    assert_eq!(
        surviving.number(),
        2,
        "logical creation identity must not become display position1"
    );
    assert_eq!(surviving.data_number(), 1);
}

#[test]
fn removing_physical_zero_compacts_the_surviving_primary_reference_without_changing_its_identity() {
    let original = raw::protected_companion(false);
    let case = dropping::reordered_physical(&original);
    let expected = dropping::after(&case);
    let mut budget = allowance(case.rows.len());
    let plan = image(&case.expected, &mut budget)
        .prepare_retained_index_drop(case.table, case.index_name, &mut budget, &|| false)
        .unwrap();
    same(plan.candidate_bytes(), &expected);
    assert_eq!(plan.logical_index_number(), 1);
    assert_eq!(plan.physical_index_number(), 0);
    let observed = crate::inspect_table_indexes(
        &mut Cursor::new(plan.candidate_bytes()),
        SCOPE,
        case.table,
        &mut budget,
        &|| false,
    )
    .unwrap();
    let [primary] = observed.indexes() else {
        panic!("only the original primary must survive");
    };
    assert_eq!(primary.number(), 0);
    assert_eq!(primary.data_number(), 0);
    let original_primary = crate::inspect_table_indexes(
        &mut Cursor::new(&original.expected),
        SCOPE,
        original.table,
        &mut budget,
        &|| false,
    )
    .unwrap();
    let original_primary = original_primary
        .indexes()
        .iter()
        .find(|index| index.number() == 0)
        .unwrap();
    assert_eq!(primary.root_page(), original_primary.root_page());
    assert_eq!(primary.usage_map(), original_primary.usage_map());
}
