//! Working public API against the previously qualified complete literal images.
use super::{budget, model};
use crate::{
    EditError, FixedUpdateImageLimits, InspectError, NativeImage, SecondaryIndexCreate,
    WriteBlocker,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(7),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn create_secondary_index_matches_all_frozen_complete_images() {
    let mut cases = 0;
    for header in [5, 6] {
        for population in model::POPULATIONS {
            for descending in [false, true] {
                let case = model::source(header, population, descending);
                let request = SecondaryIndexCreate::new(case.name, "Score", descending);
                let preview = capture(&case.before)
                    .preview_ace16_create_secondary_index("Items", &request, &mut budget(), &|| {
                        false
                    })
                    .unwrap();
                assert_eq!(preview.before_bytes(), case.before);
                assert_eq!(preview.index_name(), case.name);
                assert_eq!(preview.logical_index_number(), 4);
                assert_eq!(preview.physical_index_number(), 2);
                assert_eq!(preview.indexed_rows(), case.rows.len());
                let actual = preview.candidate_bytes();
                assert_eq!(actual.len(), case.expected.len());
                assert!(
                    actual == case.expected,
                    "header={header}, population={population:?}, descending={descending}, first mismatch={:?}",
                    actual
                        .iter()
                        .zip(&case.expected)
                        .position(|(left, right)| left != right),
                );
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 16);
}

#[test]
fn create_secondary_index_rejects_invalid_original_requests() {
    let case = model::source(5, model::Population::NullableDuplicate, false);
    for name in ["", "1Bad", "Bad Name", "Bad\0Name"] {
        let error = capture(&case.before)
            .preview_ace16_create_secondary_index(
                "Items",
                &SecondaryIndexCreate::new(name, "Score", false),
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
        assert!(matches!(
            error,
            EditError::Unsupported(WriteBlocker::Column)
        ));
    }
    let error = capture(&case.before)
        .preview_ace16_create_secondary_index(
            "Items",
            &SecondaryIndexCreate::new("PrimaryAlias", "Score", false),
            &mut budget(),
            &|| false,
        )
        .err()
        .unwrap();
    assert!(matches!(error, EditError::Conflict));

    let error = capture(&case.before)
        .preview_ace16_create_secondary_index(
            "Items",
            &SecondaryIndexCreate::new("NewId", "Id", false),
            &mut budget(),
            &|| false,
        )
        .err()
        .unwrap();
    assert!(matches!(
        error,
        EditError::Unsupported(WriteBlocker::IndexedColumn)
    ));

    let error = capture(&case.before)
        .preview_ace16_create_secondary_index(
            "Items",
            &SecondaryIndexCreate::new("NewMissing", "Missing", false),
            &mut budget(),
            &|| false,
        )
        .err()
        .unwrap();
    assert!(matches!(
        error,
        EditError::Inspect(InspectError::UnknownColumn)
    ));
}

#[test]
fn create_secondary_index_preserves_cancellation_and_resource_failure() {
    let case = model::source(6, model::Population::Pressure, true);
    let request = SecondaryIndexCreate::new(case.name, "Score", true);
    let error = capture(&case.before)
        .preview_ace16_create_secondary_index("Items", &request, &mut budget(), &|| true)
        .err()
        .unwrap();
    assert!(matches!(error, EditError::Inspect(InspectError::Cancelled)));

    let error = capture(&case.before)
        .preview_ace16_create_secondary_index(
            "Items",
            &request,
            &mut ResourceBudget::new(0, 0, 0),
            &|| false,
        )
        .err()
        .unwrap();
    assert!(matches!(error, EditError::Inspect(InspectError::Limit(_))));
}

#[test]
fn create_secondary_index_pressure_obeys_exact_pools_and_terminal_cancellation() {
    let case = model::source(6, model::Population::Pressure, true);
    let request = SecondaryIndexCreate::new(case.name, "Score", true);
    let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let limits = resources.map(|resource| budget().remaining(resource));
    let mut measured = budget();
    let polls = Cell::new(0_usize);
    let healthy = capture(&case.before)
        .preview_ace16_create_secondary_index("Items", &request, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    assert!(healthy.candidate_bytes() == case.expected);
    assert_eq!(healthy.before_bytes(), case.before);
    assert_eq!(healthy.indexed_rows(), 600);
    let terminal = polls.get();
    assert!(terminal > 1);
    let consumed =
        resources.map(|resource| budget().remaining(resource) - measured.remaining(resource));
    assert!(consumed.into_iter().all(|amount| amount > 0));

    for (position, resource) in resources.into_iter().enumerate() {
        let mut exact_limits = limits;
        exact_limits[position] = consumed[position];
        let mut exact = ResourceBudget::new(exact_limits[0], exact_limits[1], exact_limits[2]);
        let preview = capture(&case.before)
            .preview_ace16_create_secondary_index("Items", &request, &mut exact, &|| false)
            .unwrap();
        assert!(
            preview.candidate_bytes() == case.expected,
            "exact {resource:?}"
        );
        assert_eq!(exact.remaining(resource), 0);

        let mut short_limits = exact_limits;
        short_limits[position] -= 1;
        let mut short = ResourceBudget::new(short_limits[0], short_limits[1], short_limits[2]);
        let error = capture(&case.before)
            .preview_ace16_create_secondary_index("Items", &request, &mut short, &|| false)
            .err()
            .unwrap();
        assert!(
            matches!(error, EditError::Inspect(InspectError::Limit(error)) if error.resource == resource),
            "one-below {resource:?}",
        );
    }

    // Every pool can also be consumed exactly in the same successful operation.
    let mut exact = ResourceBudget::new(consumed[0], consumed[1], consumed[2]);
    let preview = capture(&case.before)
        .preview_ace16_create_secondary_index("Items", &request, &mut exact, &|| false)
        .unwrap();
    assert!(preview.candidate_bytes() == case.expected);
    assert!(
        resources
            .into_iter()
            .all(|resource| exact.remaining(resource) == 0)
    );

    // Cancel the final poll observed in the successful complete-image operation,
    // after construction and independent proof, rather than just cancelling entry.
    polls.set(0);
    let error = capture(&case.before)
        .preview_ace16_create_secondary_index("Items", &request, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == terminal
        })
        .err()
        .unwrap();
    assert!(matches!(error, EditError::Inspect(InspectError::Cancelled)));
    assert_eq!(polls.get(), terminal);
}

#[test]
fn create_secondary_index_refuses_fixed_update_purpose_even_for_empty_source() {
    for header in [5, 6] {
        for population in [model::Population::Empty, model::Population::Pressure] {
            let case = model::source(header, population, false);
            let request = SecondaryIndexCreate::new(case.name, "Score", false);
            let image = NativeImage::capture_for_fixed_updates(
                &mut Cursor::new(&case.before),
                CatalogScope::new(7),
                FixedUpdateImageLimits::new(case.before.len() as u64).unwrap(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(image.as_bytes(), case.before);

            let polls = Cell::new(0_usize);
            let mut available = budget();
            let error = image
                .preview_ace16_create_secondary_index("Items", &request, &mut available, &|| {
                    polls.set(polls.get() + 1);
                    false
                })
                .err()
                .unwrap();
            assert!(matches!(
                error,
                EditError::Inspect(InspectError::StructureLimit)
            ));
            assert_eq!(available, budget());
            assert_eq!(polls.get(), 0);
        }
    }
}
