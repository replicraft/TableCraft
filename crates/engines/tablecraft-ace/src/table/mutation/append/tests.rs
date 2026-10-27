//! Cursor and byte algebra do not grant map-cell or candidate-image authority.
use super::{State, audit_bytes, claim, clear_hint, current, merge};
use crate::{
    EditError, TableCheck, check_table,
    format::{allocation::MapCell, reader::Reader},
    test_support::encoded::model,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;

fn checked(source: &[u8]) -> TableCheck {
    check_table(
        &mut Cursor::new(source),
        CatalogScope::new(1),
        "Items",
        &mut model::budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn actual_data_index_and_bitmap_purposes_advance_one_cursor() {
    let bytes = model::text(1, false, true).bytes;
    let image = model::capture(&bytes);
    let checked = checked(&bytes);
    let index = checked.indexes()[0].index();
    let mut append = State::new(&image, 10);
    let first = (bytes.len() / PAGE) as u32;
    append
        .reserve_data(first, &mut model::budget(), &|| false)
        .unwrap();
    append
        .reserve_index_bitmap(first + 1, index, &mut model::budget(), &|| false)
        .unwrap();
    append
        .reserve_index(first + 3, index, &mut model::budget(), &|| false)
        .unwrap();
    assert_eq!(append.original_length, bytes.len());
    assert_eq!(append.length, bytes.len() + 4 * PAGE);
    assert_eq!(append.next().unwrap(), first + 4);
    assert_eq!(append.reservations_len(), 4);
    assert_eq!(
        append.indexes().map(|item| item.number).collect::<Vec<_>>(),
        [first + 1, first + 3]
    );
    append
        .require_data(first, &mut model::budget(), &|| false)
        .unwrap();
    append
        .require_index_bitmap(
            first + 2,
            index.data_number(),
            index.usage_map(),
            &mut model::budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        append.require_data(first + 1, &mut model::budget(), &|| false),
        Err(EditError::Preservation)
    );
    assert_eq!(
        append.require_index_bitmap(
            first + 2,
            index.data_number() + 1,
            index.usage_map(),
            &mut model::budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    );
    assert_eq!(
        append.reserve_data(first + 3, &mut model::budget(), &|| false),
        Err(EditError::Preservation)
    );
    assert_eq!(append.next().unwrap(), first + 4);
}

#[test]
fn one_byte_ledger_composes_roles_and_provisional_hint_clear() {
    let bytes = model::text(0, false, false).bytes;
    let image = model::capture(&bytes);
    let mut append = State::new(&image, 10);
    let mut source = image.reader();
    let mut budget = model::budget();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: &mut budget,
        cancelled: &|| false,
        reads: 0,
    };
    // Literal cells test only the paid byte primitive, not native admission.
    for mask in [1, 2, 4] {
        merge(
            &mut append.map_edits,
            MapCell {
                offset: 0,
                before: 0xff,
                mask,
            },
            false,
            &mut reader,
        )
        .unwrap();
    }
    claim(
        &mut append.map_edits,
        MapCell {
            offset: 1,
            before: 0,
            mask: 8,
        },
        true,
        &mut reader,
    )
    .unwrap();
    assert!(
        current(
            &append.map_edits,
            &MapCell {
                offset: 1,
                before: 0,
                mask: 8
            },
            &mut reader
        )
        .unwrap()
    );
    assert_eq!(
        clear_hint(
            &mut append.map_edits,
            MapCell {
                offset: 1,
                before: 8,
                mask: 8,
            },
            &mut reader,
        ),
        Err(EditError::Preservation),
    );
    assert_eq!(append.map_edits[1].after, 8);

    clear_hint(
        &mut append.map_edits,
        MapCell {
            offset: 1,
            before: 0,
            mask: 8,
        },
        &mut reader,
    )
    .unwrap();
    assert_eq!(append.map_edits.len(), 2);
    assert_eq!(
        (append.map_edits[0].before, append.map_edits[0].after),
        (0xff, 0xf8)
    );
    assert_eq!(
        (append.map_edits[1].before, append.map_edits[1].after),
        (0, 0)
    );
    audit_bytes(
        &append.map_edits,
        &[0xff, 0],
        &[0xf8, 0],
        &mut model::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        claim(
            &mut append.map_edits,
            MapCell {
                offset: 0,
                before: 0xff,
                mask: 1
            },
            false,
            &mut reader
        ),
        Err(EditError::Preservation)
    );
    assert_eq!(
        merge(
            &mut append.map_edits,
            MapCell {
                offset: 0,
                before: 0xfe,
                mask: 16
            },
            false,
            &mut reader
        ),
        Err(EditError::Preservation)
    );
}

#[test]
fn cursor_limits_and_pair_cancellation_leave_no_partial_reservation() {
    let bytes = model::binary(false, false).bytes;
    let image = model::capture(&bytes);
    let checked = checked(&bytes);
    let index = checked.indexes()[0].index();
    let first = (bytes.len() / PAGE) as u32;
    for mut limited in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        let mut append = State::new(&image, 10);
        assert!(
            append
                .reserve_index_bitmap(first, index, &mut limited, &|| false)
                .is_err()
        );
        assert_eq!(append.length, bytes.len());
        assert_eq!(append.reservations_len(), 0);
    }
    let mut append = State::new(&image, 10);
    let calls = Cell::new(0);
    assert!(
        append
            .reserve_index_bitmap(first, index, &mut model::budget(), &|| {
                calls.set(calls.get() + 1);
                calls.get() == 2
            })
            .is_err()
    );
    assert_eq!(append.length, bytes.len());
    assert_eq!(append.reservations_len(), 0);
    append
        .reserve_index_bitmap(first, index, &mut model::budget(), &|| false)
        .unwrap();
    assert_eq!(append.length, bytes.len() + 2 * PAGE);
    assert_eq!(
        append.preview(usize::MAX),
        Err(EditError::Inspect(crate::InspectError::StructureLimit))
    );
}
