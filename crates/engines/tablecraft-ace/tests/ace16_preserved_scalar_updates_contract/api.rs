//! Existing engineering fixed consumers on independently qualified rich sources.
use super::{
    SCOPE, asymmetric_guids, authority_sources, budget, catalog_flag, model, name_map,
    name_map_controls, outer_url,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    Ace16WritePreview, EditError, FixedColumnUpdates, FixedScalarUpdate, InspectError, NativeImage,
    WriteBlocker,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn request() -> [FixedScalarUpdate; 1] {
    [FixedScalarUpdate::new(
        0,
        Value::Boolean(false),
        Value::Boolean(true),
    )]
}

#[test]
fn preserved_existing_fixed_and_batch_services_match_complete_one_bit_images() {
    for case in [
        model::source(),
        catalog_flag::source(),
        name_map::source(false, false),
        name_map::source(true, false),
        asymmetric_guids::source(false),
        outer_url::source(true, false),
    ] {
        let updates = request();
        let single = capture(&case.before)
            .preview_ace16_fixed_updates("Items", "Yes_Field", &updates, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(single.before_bytes(), case.before);
        assert_eq!(single.candidate_bytes(), case.expected);
        assert_eq!(single.changed_rows(), 1);

        let groups = [FixedColumnUpdates::new("Items", "Yes_Field", &updates)];
        let batch = capture(&case.before)
            .preview_ace16_batch_updates(&groups, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(batch.candidate_bytes(), case.expected);
        assert_eq!(batch.changed_rows(), 1);

        let no_op = capture(&case.before)
            .preview_ace16_fixed_updates("Items", "Yes_Field", &[], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(no_op.candidate_bytes(), case.before);
        assert_eq!(no_op.changed_rows(), 0);
    }
}

#[test]
fn preserved_stale_carriers_unavailable_targets_and_stale_name_maps_refuse() {
    let case = catalog_flag::source();
    assert_eq!(
        capture(&case.before)
            .preview_ace16_fixed_updates(
                "Items",
                "Yes_Field",
                &[FixedScalarUpdate::new(
                    0,
                    Value::Boolean(true),
                    Value::Boolean(false)
                )],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Conflict)
    );

    for target in [
        "ID",
        "Number",
        "Short_Text",
        "Long_Text",
        "Attachment_Field",
        "Calculated",
    ] {
        assert!(
            capture(&case.before)
                .preview_ace16_fixed_updates("Items", target, &[], &mut budget(), &|| false)
                .is_err()
        );
    }
    assert!(
        capture(&case.before)
            .preview_ace16_fixed_updates(
                "Items",
                "Yes_Field",
                &[FixedScalarUpdate::new(
                    0,
                    Value::Boolean(false),
                    Value::Int32(1)
                )],
                &mut budget(),
                &|| false
            )
            .is_err()
    );

    for (case, error) in [
        (
            name_map::source(false, true),
            EditError::Unsupported(WriteBlocker::Properties),
        ),
        (
            outer_url::source(true, true),
            EditError::Unsupported(WriteBlocker::RowLayout),
        ),
    ] {
        for updates in [Vec::new(), Vec::from(request())] {
            let actual = capture(&case.before)
                .preview_ace16_fixed_updates("Items", "Yes_Field", &updates, &mut budget(), &|| {
                    false
                })
                .err();
            assert_eq!(actual.as_ref(), Some(&error));
        }
    }
}

#[test]
fn preserved_name_map_alias_foreign_and_unknown_records_refuse_even_empty_updates() {
    for case in name_map_controls::cases() {
        for updates in [Vec::new(), request().into()] {
            assert_eq!(
                capture(&case.before)
                    .preview_ace16_fixed_updates(
                        "Items",
                        "Yes_Field",
                        &updates,
                        &mut budget(),
                        &|| false
                    )
                    .err(),
                Some(EditError::Unsupported(WriteBlocker::Properties))
            );
        }
    }
}

#[test]
fn preserved_name_map_endian_mismatch_refuses_even_empty_updates() {
    let case = asymmetric_guids::source(true);
    for updates in [Vec::new(), Vec::from(request())] {
        assert_eq!(
            capture(&case.before)
                .preview_ace16_fixed_updates("Items", "Yes_Field", &updates, &mut budget(), &|| {
                    false
                })
                .err(),
            Some(EditError::Unsupported(WriteBlocker::Properties))
        );
    }
}

#[test]
fn preserved_index_ownership_and_global_allocation_refuse_before_even_empty_updates() {
    for contradiction in [
        authority_sources::Contradiction::UnreachableIndexOwnedData,
        authority_sources::Contradiction::FreeOrdinaryData,
        authority_sources::Contradiction::FreeIndexPage,
    ] {
        let case = authority_sources::source(contradiction);
        for updates in [Vec::new(), Vec::from(request())] {
            let result = capture(&case.before).preview_ace16_fixed_updates(
                "Items",
                "Yes_Field",
                &updates,
                &mut budget(),
                &|| false,
            );
            match contradiction {
                authority_sources::Contradiction::UnreachableIndexOwnedData => {
                    assert_eq!(
                        result.err(),
                        Some(EditError::Unsupported(WriteBlocker::RowLayout))
                    );
                }
                authority_sources::Contradiction::FreeOrdinaryData
                | authority_sources::Contradiction::FreeIndexPage => {
                    assert_eq!(
                        result.err(),
                        Some(EditError::Inspect(InspectError::Malformed))
                    );
                }
            }
        }
    }
}

fn cumulative(
    before: &[u8],
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let image = NativeImage::capture(&mut Cursor::new(before), SCOPE, work, cancelled)?;
    image.preview_ace16_fixed_updates("Items", "Yes_Field", &request(), work, cancelled)
}

#[test]
fn preserved_capture_through_proof_exact_pools_one_below_and_final_cancel() {
    let case = catalog_flag::source();
    let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let limits = [128 * 1024 * 1024, 100_000, 10_000_000];
    let mut work = ResourceBudget::new(limits[0], limits[1], limits[2]);
    let polls = Cell::new(0);
    let copy = cumulative(&case.before, &mut work, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(copy.candidate_bytes(), case.expected);
    let needed =
        std::array::from_fn::<_, 3, _>(|index| limits[index] - work.remaining(resources[index]));
    let exact = cumulative(
        &case.before,
        &mut ResourceBudget::new(needed[0], needed[1], needed[2]),
        &|| false,
    )
    .unwrap();
    assert_eq!(exact.candidate_bytes(), case.expected);
    for index in 0..3 {
        assert!(needed[index] > 0);
        let mut quota = limits;
        quota[index] = needed[index] - 1;
        assert!(
            matches!(cumulative(&case.before, &mut ResourceBudget::new(quota[0], quota[1], quota[2]), &|| false),
            Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == resources[index])
        );
    }
    let final_poll = polls.get();
    let polls = Cell::new(0);
    assert!(matches!(
        cumulative(&case.before, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == final_poll
        }),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
}
