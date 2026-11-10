//! Independently authored candidate bitmaps qualify observation, not old writers.
use crate::{
    CreateColumn, EditError, FirstTableDefinition, InspectError, NativeImage, WriteBlocker,
    table::{mutation::catalog_property::ledger, table_append::Inventory},
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table as append, canonical_wide_table_creation::property_windows as raw,
};

const SCOPE: CatalogScope = CatalogScope::new(7391);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 10_000_000, 512 * 1024 * 1024)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn native_creation_observes_indirect_candidates_and_preserves_them_on_next_append() {
    let inline = raw::source();
    let inline_image = capture(&inline.after);
    ledger::capture_retained(
        &inline.after,
        inline_image.catalog(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let source = raw::indirect_candidates();
    let image = capture(&source.after);
    assert!(matches!(
        ledger::capture_retained(&source.after, image.catalog(), &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
    ledger::capture_represented(&source.after, image.catalog(), &mut budget(), &|| false).unwrap();
    Inventory::capture_packed(&image, &mut budget(), &|| false).unwrap();
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("Score", ScalarType::Int32),
    ];
    let definition =
        FirstTableDefinition::capture_native("Next", &columns, &mut budget(), &|| false).unwrap();
    let expected = append::append(&source.after, "Next", append::Target::GeneratedNumber);
    let plan = image
        .plan_add_table_native(definition, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes().len(), expected.len());
    assert_eq!(
        plan.after_bytes()
            .iter()
            .zip(&expected)
            .position(|(actual, expected)| actual != expected),
        None
    );
    let advice = crate::authored_add_table_source_capabilities(
        plan.after_bytes(),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(advice.can_add_keyed());
    assert!(advice.can_add_keyless());
    assert!(advice.requires_native());
}

#[test]
fn native_candidate_observation_requires_owned_members_allocated_storage_and_unaliased_refs() {
    for case in [
        "outside-owned",
        "free-bitmap",
        "bad-signature",
        "aliased-refs",
    ] {
        let mut source = raw::indirect_candidates();
        match case {
            "outside-owned" => source.after[95 * 4096 + 4 + 17 / 8] |= 1 << (17 % 8),
            "free-bitmap" => source.after[2 * 4096 - 1] |= 0x80,
            "bad-signature" => source.after[95 * 4096] = 4,
            "aliased-refs" => {
                // Actual catalog LvProp pair: native14, owned3:16, hint3:17.
                let pattern = [14, 0, 16, 3, 0, 0, 17, 3, 0, 0];
                let page = &mut source.after[2 * 4096..3 * 4096];
                let matches = page
                    .windows(pattern.len())
                    .enumerate()
                    .filter_map(|(position, bytes)| (bytes == pattern).then_some(position))
                    .collect::<Vec<_>>();
                assert_eq!(matches.len(), 1);
                page[matches[0] + 6..matches[0] + 10].copy_from_slice(&[16, 3, 0, 0]);
            }
            _ => unreachable!(),
        }
        let image = capture(&source.after);
        let result = Inventory::capture_packed(&image, &mut budget(), &|| false);
        if case == "aliased-refs" {
            assert!(matches!(
                result,
                Err(EditError::Unsupported(WriteBlocker::RowLayout))
            ));
        } else {
            assert!(matches!(
                result,
                Err(EditError::Inspect(InspectError::Malformed))
            ));
        }
    }
    let source = raw::indirect_candidates();
    let image = capture(&source.after);
    assert!(matches!(
        Inventory::capture_packed(&image, &mut budget(), &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    for (resource, mut limited) in [
        (
            Resource::Bytes,
            ResourceBudget::new(0, 10_000_000, 512 * 1024 * 1024),
        ),
        (
            Resource::Rows,
            ResourceBudget::new(512 * 1024 * 1024, 0, 512 * 1024 * 1024),
        ),
        (
            Resource::Steps,
            ResourceBudget::new(512 * 1024 * 1024, 10_000_000, 0),
        ),
    ] {
        assert!(matches!(
            Inventory::capture_packed(&image, &mut limited, &|| false),
            Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == resource
        ));
    }
}
