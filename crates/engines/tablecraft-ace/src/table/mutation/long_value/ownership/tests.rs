//! Final certification rebinds map authority to the immutable original catalog.
use super::super::{Operation, admission, finish, stage, tests::model};
use crate::{
    EditError, InspectError, WriteBlocker,
    table::mutation::map_authority::{self, Owner},
    test_support::support,
};
use tablecraft_core::values::Value;

fn original(shared: bool) -> Vec<u8> {
    let mut bytes = model::source(6, 1);
    model::set_maps(&mut bytes, &[], &[], &[model::OLE]);
    let row = model::record(
        1,
        Some(&model::inline(&[65, 0])),
        Some(&model::single(1, model::OLE, 0)),
    );
    model::install(&mut bytes, model::ROOT, &model::page(10, &[(0, row)]));
    model::other_table_alias(&mut bytes, true, false);
    if shared {
        support::put32(&mut bytes, 15 * 4096 + 131, (11 << 8) | 2);
        support::put32(&mut bytes, 15 * 4096 + 135, (11 << 8) | 3);
    }
    bytes
}

#[test]
fn finalizer_rechecks_empty_map_identity_against_the_original_catalog() {
    let healthy = original(false);
    let requests = [model::memo_update(Value::Text("Ā".repeat(33)))];
    let image = model::capture(&healthy);
    let prepared =
        admission::prepare(&image, "Items", "Memo", &mut model::budget(), &|| false).unwrap();
    let staged = stage::build(&image, &prepared, &requests, &mut model::budget(), &|| {
        false
    })
    .unwrap();
    let operation = Operation {
        table: "Items",
        column: "Memo",
        updates: &requests,
    };
    let candidate = finish::finish(
        model::capture(&healthy),
        &operation,
        &staged,
        &mut model::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        model::read(candidate.candidate_bytes())[0][1],
        Value::Text("Ā".repeat(33))
    );

    // The selected row/map before-bytes still match this staged plan, while the
    // original unrelated declaration now also claims its empty mutable maps.
    let shared = original(true);
    assert_eq!(model::check(&shared).columns()[0].owned_pages(), 0);
    assert_eq!(
        finish::finish(
            model::capture(&shared),
            &operation,
            &staged,
            &mut model::budget(),
            &|| false
        )
        .err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );
}

#[test]
fn readable_unreferenced_physical_slot_does_not_establish_complete_map_authority() {
    let healthy = model::source(6, 1);
    let image = model::capture(&healthy);
    super::Inventory::capture(&image, &mut model::budget(), &|| false).unwrap();

    let mut hole = healthy.clone();
    let definition = &mut hole[10 * 4096..11 * 4096];
    // Literal descriptor widths are independent of the implementation. One
    // orphan physical slot adds its 12-byte pre-column record and 52-byte body.
    definition.copy_within(63..4096 - 12, 75);
    definition[63..75].fill(0);
    let index_start = model::DECLARATIONS + 12;
    definition.copy_within(index_start..4096 - 52, index_start + 52);
    definition[index_start..index_start + 52].fill(0);
    support::put32(definition, 51, 1);
    // The hidden descriptor claims a real map row even without a logical name.
    support::put32(definition, index_start + 34, (11 << 8) | 2);
    assert_eq!(model::read(&hole), model::read(&healthy));
    assert_eq!(model::check(&hole).columns()[0].owned_pages(), 1);
    let image = model::capture(&hole);
    assert_eq!(
        super::Inventory::capture(&image, &mut model::budget(), &|| false).err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );
    let request = [model::memo_update(Value::Text("B".into()))];
    assert_eq!(
        image
            .preview_ace16_long_value_updates(
                "Items",
                "Memo",
                &request,
                &mut model::budget(),
                &|| false,
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );
}

#[test]
fn metadata_footprints_do_not_grant_payload_membership_or_write_authority() {
    let mut bytes = model::source(6, 1);
    bytes[model::MEMO * 4096..model::MEMO * 4096 + 2].fill(0);
    let image = model::capture(&bytes);
    let census = map_authority::capture_catalog(
        &image,
        &mut model::budget(),
        &|| false,
        |_, _, _, _| Ok(()),
    )
    .unwrap();
    census
        .require_exclusive(&[Owner::GlobalFree], &mut model::budget(), &|| false)
        .unwrap();
    assert_eq!(
        super::Inventory::capture(&image, &mut model::budget(), &|| false).err(),
        Some(EditError::Inspect(InspectError::Malformed))
    );
    assert_eq!(
        image
            .preview_ace16_long_value_updates(
                "Items",
                "Memo",
                &[model::memo_update(Value::Text("B".into()))],
                &mut model::budget(),
                &|| false,
            )
            .err(),
        Some(EditError::Inspect(InspectError::Malformed))
    );
}
