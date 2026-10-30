use super::*;
use tablecraft_testkit::access::map_growth::model::number;

const SLOT_START: usize = 14;
const SLOT_BYTES: usize = 2;
const BITMAP_POINTER: usize = 1;

fn selected_before(model: &model::Model, logical_indexes: usize) {
    let rows = read_table(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Items",
        &mut fixed_model::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rows.rows().len(), model::ROWS);
    let indexes = check_table(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Items",
        &mut fixed_model::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(indexes.indexes().len(), logical_indexes);
    assert!(
        indexes
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
}

#[test]
fn cross_owner_current_and_historical_map_references_do_not_confer_mutable_authority() {
    for alter in [
        model::bind_other_table_hint as fn(&mut model::Model),
        model::bind_historical_owned,
    ] {
        let mut model = model::source(6);
        alter(&mut model);
        before(&model, 4);
        let original = model.bytes.clone();
        assert_eq!(
            run(&model).err(),
            Some(EditError::Unsupported(WriteBlocker::RowLayout))
        );
        assert_eq!(model.bytes, original);
    }
    // This is a raw role-alias discriminator. Once the foreign ordinary map
    // contains an index root it is not a healthy writable foreign table.
    let mut raw = model::source(6);
    model::bind_other_table_owned(&mut raw);
    selected_before(&raw, 4);
    assert_eq!(
        read_table(
            &mut Cursor::new(&raw.bytes),
            CatalogScope::new(1),
            "Other",
            &mut fixed_model::budget(),
            &|| false,
        )
        .err(),
        Some(InspectError::Malformed),
    );
    assert_eq!(
        run(&raw).err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );
}

#[test]
fn foreign_bitmap_storage_cannot_alias_selected_global_storage() {
    let mut hint = model::source(6);
    let mut shared = vec![0; model::MAP_BYTES];
    shared[0] = 1;
    support::put32(&mut shared, BITMAP_POINTER, model::GLOBAL_BITMAP as u32);
    model::install(
        &mut hint.bytes,
        model::OTHER_MAP,
        &support::data_page(
            0,
            &[
                (0, model::map(&[])),
                (0, shared),
                (0, model::map(&[])),
                (0, model::map(&[])),
            ],
        ),
    );
    before(&hint, 4);
    assert_eq!(
        run(&hint).err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );

    let mut raw_owned = model::source(6);
    model::bind_other_bitmap_to_global(&mut raw_owned);
    selected_before(&raw_owned, 4);
    assert_eq!(
        read_table(
            &mut Cursor::new(&raw_owned.bytes),
            CatalogScope::new(1),
            "Other",
            &mut fixed_model::budget(),
            &|| false,
        )
        .err(),
        Some(InspectError::Malformed),
    );
    assert_eq!(
        run(&raw_owned).err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );
}

#[test]
fn incomplete_inventory_and_unreferenced_physical_slots_cannot_disappear() {
    let mut unknown = model::source(6);
    model::unknown_other_history(&mut unknown);
    before(&unknown, 4);
    assert_eq!(
        run(&unknown).err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );

    for alias in [false, true] {
        let mut orphan = model::source(6);
        model::unreferenced_physical_alias(&mut orphan);
        if !alias {
            let position = model::selected_usage_position(&orphan.bytes, 1);
            support::put32(&mut orphan.bytes, position, (model::MAPS[1] as u32) << 8);
        }
        before(&orphan, 2);
        // Selected-table physical coverage predates the shared catalog census
        // and refuses this orphan before allocating or inspecting foreign roles.
        assert_eq!(
            run(&orphan).err(),
            Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
        );
    }
    // Adjacent native slot ranges cannot overlap healthily. A zero-length map
    // range is parser corruption, not a fabricated healthy exclusivity case.
    let mut malformed = model::source(6);
    let start = number(
        &malformed.bytes,
        model::OTHER_MAP * model::PAGE + SLOT_START,
        SLOT_BYTES,
    );
    support::put16(
        &mut malformed.bytes,
        model::OTHER_MAP * model::PAGE + SLOT_START + SLOT_BYTES,
        start as u16,
    );
    selected_before(&malformed, 4);
    assert_eq!(
        run(&malformed).err(),
        Some(EditError::Inspect(InspectError::Malformed))
    );
    let healthy = model::source(6);
    before(&healthy, 4);
    verify(&healthy, &run(&healthy).unwrap());
}
