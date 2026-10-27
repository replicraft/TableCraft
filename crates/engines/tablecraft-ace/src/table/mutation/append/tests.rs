//! Cursor and byte algebra do not grant map-cell or candidate-image authority.
use super::{Edit, State, audit_bytes, claim, clear_hint, current, merge, rewrite_record};
use crate::{
    EditError, InspectError, NativeImage, TableCheck, check_table,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapCell, MapRole, RepresentedMap},
        reader::Reader,
    },
    test_support::encoded::{model, split_model},
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
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

fn rewrite_fixture() -> (NativeImage, usize, Vec<u8>) {
    let bytes = split_model::source(split_model::Family::Binary, false, true).bytes;
    let checked = checked(&bytes);
    let reference = checked.indexes()[0].index().usage_map();
    let image = model::capture(&bytes);
    let record = {
        let mut source = image.reader();
        let mut budget = model::budget();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget: &mut budget,
            cancelled: &|| false,
            reads: 0,
        };
        let container = reference >> 8;
        let page = reader.page(container).unwrap();
        reader
            .row_location(container, reference as u8, 0, &page)
            .unwrap()
    };
    assert_eq!(record.data.len(), 69);
    assert_eq!(&record.data[..5], &[0, 0, 0, 0, 0]);
    assert_eq!(record.data[6], 0x20);
    assert_eq!(record.data[68], 0);
    (image, record.offset, record.data)
}

fn primed_record_cells(image: &NativeImage, offset: usize, before: &[u8]) -> Vec<Edit> {
    let mut source = image.reader();
    let mut budget = model::budget();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: &mut budget,
        cancelled: &|| false,
        reads: 0,
    };
    let global =
        RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree).unwrap();
    let mut edits = Vec::new();
    // Two allocations share one global byte; the third uses another byte.
    // These are byte-primitive inputs, not native allocation/admission claims.
    for number in [24, 25, 32] {
        let cell = global.future_cell(&mut reader, number).unwrap();
        assert_ne!(cell.before & cell.mask, 0);
        merge(&mut edits, cell, false, &mut reader).unwrap();
    }
    // Stage page510 in the original base0 row. A subsequent rebase must move
    // its membership, rather than replay this original-offset cell afterward.
    merge(
        &mut edits,
        MapCell {
            offset: offset + 68,
            before: before[68],
            mask: 0x40,
        },
        true,
        &mut reader,
    )
    .unwrap();
    assert_eq!(edits.len(), 3);
    edits
}

fn cell_values(edits: &[Edit]) -> Vec<(usize, u8, u8)> {
    edits
        .iter()
        .map(|edit| (edit.offset, edit.before, edit.after))
        .collect()
}

fn rebased_record() -> Vec<u8> {
    let mut after = vec![0; 69];
    after[1..5].copy_from_slice(&8_u32.to_le_bytes());
    // Exact independent base8 memberships13,510,512.
    after[5] = 0x20;
    after[67] = 0x40;
    after[68] = 0x01;
    after
}

#[test]
fn whole_record_rewrite_absorbs_only_its_cells_and_keeps_global_composition() {
    let (image, offset, before) = rewrite_fixture();
    let mut edits = primed_record_cells(&image, offset, &before);
    let outside = cell_values(&edits)
        .into_iter()
        .filter(|(at, _, _)| *at < offset || *at >= offset + before.len())
        .collect::<Vec<_>>();
    assert_eq!(outside.len(), 2);
    let after = rebased_record();
    let mut source = image.reader();
    let mut budget = model::budget();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: &mut budget,
        cancelled: &|| false,
        reads: 0,
    };
    rewrite_record(&mut edits, offset, &before, &after, &mut reader).unwrap();
    assert_eq!(edits.len(), before.len() + outside.len());
    assert_eq!(
        cell_values(&edits)
            .into_iter()
            .filter(|(at, _, _)| *at < offset || *at >= offset + before.len())
            .collect::<Vec<_>>(),
        outside,
    );
    for (position, (&old, &new)) in before.iter().zip(&after).enumerate() {
        let edit = edits
            .iter()
            .find(|edit| edit.offset == offset + position)
            .unwrap();
        assert_eq!((edit.before, edit.after), (old, new));
    }
    assert_eq!(
        edits
            .iter()
            .find(|edit| edit.offset == offset + 68)
            .unwrap()
            .after,
        1,
    );
    assert_eq!(
        claim(
            &mut edits,
            MapCell {
                offset: offset + 68,
                before: before[68],
                mask: 0x40,
            },
            true,
            &mut reader,
        ),
        Err(EditError::Preservation),
    );

    // Rewriting the same record again still binds ORIGINAL source bytes,
    // while replacing the entire prior after state. Outside bytes stay exact.
    let mut promoted = vec![0; 69];
    promoted[0] = 1;
    promoted[1..5].copy_from_slice(&27_u32.to_le_bytes());
    rewrite_record(&mut edits, offset, &before, &promoted, &mut reader).unwrap();
    let mut byte_image = image.as_bytes().to_vec();
    for edit in &edits {
        byte_image[edit.offset] = edit.after;
    }
    assert_eq!(&byte_image[offset..offset + before.len()], &promoted);
    assert_eq!(
        cell_values(&edits)
            .into_iter()
            .filter(|(at, _, _)| *at < offset || *at >= offset + before.len())
            .collect::<Vec<_>>(),
        outside,
    );
    audit_bytes(
        &edits,
        image.as_bytes(),
        &byte_image,
        &mut model::budget(),
        &|| false,
    )
    .unwrap();
    // No candidate/check_table assertion is made for the arbitrary rewritten
    // metadata: the caller still owns complete framing and role authority.
}

#[test]
fn record_rewrite_rejects_conflicting_before_shapes_and_limits_atomically() {
    let (image, offset, before) = rewrite_fixture();
    let after = rebased_record();
    for mode in 0..9 {
        let mut edits = primed_record_cells(&image, offset, &before);
        let original = cell_values(&edits);
        let mut old = before.clone();
        let mut replacement = after.clone();
        let mut target = offset;
        let mut budget = match mode {
            4 => ResourceBudget::new(0, 100_000, 30_000_000),
            5 => ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
            6 => ResourceBudget::new(128 * 1024 * 1024, 100_000, 200),
            _ => model::budget(),
        };
        match mode {
            0 => old[68] ^= 1,
            1 => {
                replacement.pop().unwrap();
            }
            2 => target = usize::MAX - 10,
            3 => {
                old.resize(PAGE + 1, 0);
                replacement.resize(PAGE + 1, 0);
            }
            _ => {}
        }
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            match mode {
                7 => calls.get() == 1,
                8 => calls.get() == 20,
                _ => false,
            }
        };
        let error = {
            let mut source = image.reader();
            let mut reader = Reader {
                source: &mut source,
                header: image.catalog().header(),
                budget: &mut budget,
                cancelled: &cancelled,
                reads: 0,
            };
            rewrite_record(&mut edits, target, &old, &replacement, &mut reader).unwrap_err()
        };
        match mode {
            4 => assert!(matches!(
                error,
                EditError::Inspect(InspectError::Limit(LimitError {
                    resource: Resource::Bytes,
                    ..
                }))
            )),
            5 | 6 => assert!(matches!(
                error,
                EditError::Inspect(InspectError::Limit(LimitError {
                    resource: Resource::Steps,
                    ..
                }))
            )),
            7 | 8 => assert_eq!(error, EditError::Inspect(InspectError::Cancelled)),
            _ => assert_eq!(error, EditError::Preservation),
        }
        assert_eq!(cell_values(&edits), original, "mode {mode}");
        let mut source = image.reader();
        let mut retry_budget = model::budget();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget: &mut retry_budget,
            cancelled: &|| false,
            reads: 0,
        };
        rewrite_record(&mut edits, offset, &before, &after, &mut reader).unwrap();
        assert_eq!(edits.len(), 71);
    }
}

#[test]
fn ordinary_candidate_bitmap_identity_is_not_data_or_index_membership() {
    let bytes = model::text(1, false, true).bytes;
    let image = model::capture(&bytes);
    let checked = checked(&bytes);
    let index = checked.indexes()[0].index();
    let definition = checked.schema().definition_page();
    let reference = index.usage_map();
    let first = (bytes.len() / PAGE) as u32;
    let mut append = State::new(&image, definition);
    append
        .reserve_data(first, &mut model::budget(), &|| false)
        .unwrap();
    // This real parsed reference is used solely as a tuple identity. The
    // fixture's index-owned map is NOT asserted to be an ordinary-candidate
    // declaration: the caller's census/role proof remains required.
    append
        .reserve_candidate_bitmap(first + 1, reference, &mut model::budget(), &|| false)
        .unwrap();
    append
        .reserve_index_bitmap(first + 2, index, &mut model::budget(), &|| false)
        .unwrap();
    append
        .require_candidate_bitmap(
            first + 1,
            definition,
            reference,
            &mut model::budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(append.next().unwrap(), first + 4);
    assert_eq!(append.reservations_len(), 4);
    assert_eq!(
        append.indexes().map(|item| item.number).collect::<Vec<_>>(),
        [first + 2],
    );
    for number in [first, first + 2, first + 3] {
        assert_eq!(
            append.require_candidate_bitmap(
                number,
                definition,
                reference,
                &mut model::budget(),
                &|| false,
            ),
            Err(EditError::Preservation),
        );
    }
    for (owner, usage) in [(definition + 1, reference), (definition, reference ^ 1)] {
        assert_eq!(
            append.require_candidate_bitmap(
                first + 1,
                owner,
                usage,
                &mut model::budget(),
                &|| false,
            ),
            Err(EditError::Preservation),
        );
    }
    assert_eq!(
        append.require_data(first + 1, &mut model::budget(), &|| false),
        Err(EditError::Preservation),
    );
    assert_eq!(
        append.require_index_bitmap(
            first + 1,
            index.data_number(),
            reference,
            &mut model::budget(),
            &|| false,
        ),
        Err(EditError::Preservation),
    );
}

#[test]
fn candidate_bitmap_reservation_pool_and_cancel_failures_preserve_cursor() {
    let bytes = model::text(1, false, true).bytes;
    let image = model::capture(&bytes);
    let checked = checked(&bytes);
    let reference = checked.indexes()[0].index().usage_map();
    let first = (bytes.len() / PAGE) as u32;
    for mode in 0..3 {
        let mut append = State::new(&image, checked.schema().definition_page());
        let mut budget = match mode {
            0 => ResourceBudget::new(0, 100_000, 30_000_000),
            1 => ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
            _ => model::budget(),
        };
        let error = append
            .reserve_candidate_bitmap(first, reference, &mut budget, &|| mode == 2)
            .unwrap_err();
        match mode {
            0 => assert!(matches!(
                error,
                EditError::Inspect(InspectError::Limit(LimitError {
                    resource: Resource::Bytes,
                    ..
                }))
            )),
            1 => assert!(matches!(
                error,
                EditError::Inspect(InspectError::Limit(LimitError {
                    resource: Resource::Steps,
                    ..
                }))
            )),
            _ => assert_eq!(error, EditError::Inspect(InspectError::Cancelled)),
        }
        assert_eq!(append.length, bytes.len());
        assert_eq!(append.reservations_len(), 0);
        assert!(append.indexes().next().is_none());
        append
            .reserve_candidate_bitmap(first, reference, &mut model::budget(), &|| false)
            .unwrap();
        assert_eq!(append.next().unwrap(), first + 1);
        assert_eq!(append.reservations_len(), 1);
        assert!(append.indexes().next().is_none());
    }
}
