//! Correlated staged facts cannot replace original caller/source authority.
use super::prove;
use crate::{
    EditError, NativeImage, SecondaryIndexCreate,
    table::mutation::{
        create_index_tests::model,
        index_create::{source, stage},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 200_000, 32_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(7),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn prepared(case: &model::Case) -> (NativeImage, stage::Plan, Vec<u8>) {
    let image = capture(&case.before);
    let request = SecondaryIndexCreate::new(case.name, "Score", case.descending);
    let original = source::qualify(&image, "Items", &request, &mut budget(), &|| false).unwrap();
    let plan = stage::prepare(&image, "Items", &request, &original, &mut budget(), &|| {
        false
    })
    .unwrap();
    let after = stage::compose(&image, &original, &plan, &mut budget(), &|| false).unwrap();
    assert!(
        after == case.expected,
        "healthy plan must match frozen whole-image authority"
    );
    assert_eq!(
        prove(
            &image,
            "Items",
            &request,
            &plan,
            &after,
            &mut budget(),
            &|| false
        )
        .unwrap(),
        case.rows.len(),
    );
    (image, plan, after)
}

#[test]
fn original_direction_rejects_an_alternative_valid_population_and_candidate() {
    let case = model::source(5, model::Population::Pressure, true);
    let (image, plan, after) = prepared(&case);
    let original_request = SecondaryIndexCreate::new(case.name, "Score", false);
    assert!(matches!(
        prove(
            &image,
            "Items",
            &original_request,
            &plan,
            &after,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation),
    ));
}

#[test]
fn correlated_fresh_history_and_descriptor_do_not_replace_original_families() {
    let case = model::source(6, model::Population::NullableDuplicate, false);
    let (image, mut plan, mut after) = prepared(&case);
    let request = SecondaryIndexCreate::new(case.name, "Score", false);
    plan.distinct += 1;
    let original = source::qualify(&image, "Items", &request, &mut budget(), &|| false).unwrap();
    let counter_offset = original.layout.columns.start + size_of::<u32>();
    plan.definition[counter_offset..counter_offset + size_of::<u32>()]
        .copy_from_slice(&plan.distinct.to_le_bytes());
    let absolute = original.layout.definition as usize * model::PAGE + counter_offset;
    after[absolute..absolute + size_of::<u32>()].copy_from_slice(&plan.distinct.to_le_bytes());
    assert!(matches!(
        prove(
            &image,
            "Items",
            &request,
            &plan,
            &after,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation),
    ));
}

#[test]
fn correlated_missing_global_claims_do_not_validate_allocated_suffix() {
    let case = model::source(5, model::Population::Distinct, false);
    let (image, mut plan, mut after) = prepared(&case);
    let request = SecondaryIndexCreate::new(case.name, "Score", false);
    for edit in &plan.append.map_edits {
        after[edit.offset] = edit.before;
    }
    plan.append.map_edits.clear();
    assert!(matches!(
        prove(
            &image,
            "Items",
            &request,
            &plan,
            &after,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation),
    ));
}
