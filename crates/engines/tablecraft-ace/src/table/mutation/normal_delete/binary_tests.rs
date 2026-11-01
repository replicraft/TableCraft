//! Direct frozen-image DELETE proof, independent of staging and normal planners.
#[path = "../../../../tests/qualified_recovery/api/normal/genuine_binary_delete/fixtures.rs"]
mod fixtures;

use super::proof;
use crate::{EditError, NativeEditPlan, RowDelete};
use fixtures::{Case, Recipe};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(218);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn plan(case: &Case) -> NativeEditPlan {
    NativeEditPlan {
        before: case.before.clone(),
        after: case.after.clone(),
        changed_rows: case.recipe.selected().len(),
    }
}

fn requests(case: &Case, reverse: bool) -> Vec<RowDelete> {
    fixtures::witnesses(case, reverse)
        .into_iter()
        .map(|(ordinal, raw)| RowDelete::new(ordinal, raw))
        .collect()
}

fn prove(plan: &NativeEditPlan, rows: &[RowDelete]) -> Result<(), EditError> {
    proof::prove(plan, SCOPE, "BinaryKeys", rows, &mut budget(), &|| false)
}

fn segment_offsets(case: &Case) -> Vec<usize> {
    if matches!(case.recipe, Recipe::Segments | Recipe::All) {
        return Vec::new();
    }
    let root = case.roots[2] as usize * 4096;
    let mut start = root + 480;
    let mut continued = None;
    let mut padding = None;
    for bit in 0..453 * 8 {
        if case.after[root + 27 + bit / 8] & (1 << (bit % 8)) == 0 {
            continue;
        }
        let end = root + 480 + bit;
        let component = end - start - 4;
        if component > 10 && continued.is_none() {
            assert_eq!(case.after[start + 9], 9);
            continued = Some(start + 9);
        }
        if component > 1 && padding.is_none() {
            assert_eq!((component - 1) % 9, 0);
            let used = usize::from(!case.after[end - 5]);
            assert!((1..=8).contains(&used));
            if used < 8 {
                let at = start + 1 + component - 10 + used;
                assert_eq!(case.after[at], 255);
                padding = Some(at);
            }
        }
        start = end;
    }
    vec![continued.unwrap(), padding.unwrap()]
}

#[test]
#[ignore = "requires fourteen native/SDK-qualified Binary DELETE source views"]
fn literal_binary_delete_proof_accepts_all_sources_reverse_orders_and_empty_tables() {
    let mut accepted = 0;
    for case in fixtures::cases() {
        prove(&plan(&case), &requests(&case, false)).unwrap();
        accepted += 1;
        if case.recipe.selected().len() > 1 {
            prove(&plan(&case), &requests(&case, true)).unwrap();
            accepted += 1;
        }
        let empty = NativeEditPlan {
            before: case.after.clone(),
            after: case.after.clone(),
            changed_rows: 0,
        };
        prove(&empty, &[]).unwrap();
        accepted += 1;
    }
    assert_eq!(accepted, 30);
}

#[test]
#[ignore = "requires fourteen native/SDK-qualified Binary DELETE source views"]
fn binary_delete_direct_proof_rejects_masked_data_row_history_map_and_inactive_tail_changes() {
    let mut refused = 0;
    for case in fixtures::cases() {
        let rows = requests(&case, false);
        let mut offsets = fixtures::forged_offsets(&case);
        offsets.extend(segment_offsets(&case));
        for at in offsets {
            let mut forged = plan(&case);
            forged.after[at] ^= 1;
            assert_eq!(prove(&forged, &rows), Err(EditError::Preservation));
            refused += 1;
        }
        let mut count = plan(&case);
        count.changed_rows += 1;
        assert_eq!(prove(&count, &rows), Err(EditError::Preservation));
        refused += 1;

        // This original differs only in a valid opaque INDEX tail byte. A proof
        // must preserve that actual BEFORE, rather than a fixture hash/template.
        let mut origin = plan(&case);
        origin.before[case.roots[2] as usize * 4096 + 4095] ^= 1;
        assert_eq!(prove(&origin, &rows), Err(EditError::Preservation));
        refused += 1;
    }
    assert_eq!(refused, 160);
}

#[test]
#[ignore = "requires fourteen native/SDK-qualified Binary DELETE source views"]
fn binary_delete_direct_proof_requires_the_exact_original_masked_raw_witness() {
    for case in fixtures::cases() {
        let first = case.recipe.selected()[0];
        let mut stale = fixtures::record(first);
        *stale.last_mut().unwrap() ^= 2;
        let mut literal = plan(&case);
        literal.changed_rows = 1;
        assert_eq!(
            prove(&literal, &[RowDelete::new(first, stale)]),
            Err(EditError::Conflict),
        );
    }
}
