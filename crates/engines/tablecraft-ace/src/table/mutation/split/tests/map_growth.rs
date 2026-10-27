//! Fully staged controls precede independent metadata-purpose forgeries.
use super::super::{Staged, finish, fixed::FixedOperation, preflight, stage_checked};
use super::capture;
use crate::{
    Ace16WritePreview, EditError, FixedScalarUpdate, IndexCheckStatus, InspectError, check_table,
    test_support::{fixed_split::model as fixed_model, map_growth::model},
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

fn forged(mut alter: impl FnMut(&mut Staged)) -> Result<Ace16WritePreview, EditError> {
    let mut source = model::source(6);
    // A preexisting empty second-span bitmap has no members. Referencing it
    // changes pointer authority without changing ordinary reader membership.
    source.bytes[521 * 4096..521 * 4096 + 4].copy_from_slice(&[5, 1, 0, 0]);
    let requests = [FixedScalarUpdate::new(
        0,
        Value::Int32(0),
        Value::Int32(i32::MAX),
    )];
    let operation = FixedOperation {
        table: "Items",
        column: "Score",
        requests: &requests,
    };
    let image = capture(&source.bytes);
    let structural = preflight(&image, &operation, &mut fixed_model::budget(), &|| false)?;
    let mut staged = stage_checked::<4>(
        &image,
        &operation,
        structural,
        &mut fixed_model::budget(),
        &|| false,
    )?;
    let valid = finish::bound(
        capture(&source.bytes),
        &operation,
        &staged,
        &mut fixed_model::budget(),
        &|| false,
    )?;
    assert_eq!(valid.candidate_bytes().len(), source.bytes.len() + 6 * 4096);
    alter(&mut staged);
    finish::bound(
        image,
        &operation,
        &staged,
        &mut fixed_model::budget(),
        &|| false,
    )
}

#[test]
fn bitmap_membership_metadata_exclusion_and_unused_bytes_are_independently_certified() {
    for (offset, mask, expected) in [
        (4 + 601 / 8, 1 << (601 % 8), EditError::Preservation),
        (4095, 1, EditError::Inspect(InspectError::Malformed)),
        (4 + 19 / 8, 1 << (19 % 8), EditError::Preservation),
    ] {
        assert_eq!(
            forged(|staged| {
                let map = &mut staged.plan.promotions[0];
                map.bitmap[offset] ^= mask;
                let added = staged
                    .plan
                    .added
                    .iter_mut()
                    .find(|page| page.number == 601)
                    .unwrap();
                added.bytes[offset] ^= mask;
            })
            .err(),
            Some(expected),
            "bitmap offset {offset}",
        );
    }
}

#[test]
fn appended_data_impersonation_cannot_pass_the_index_or_usage_purpose_proof() {
    for number in [600, 601, 602] {
        assert!(
            forged(|staged| {
                let page = staged
                    .plan
                    .added
                    .iter_mut()
                    .find(|page| page.number == number)
                    .unwrap();
                page.bytes[..2].copy_from_slice(&[1, 1]);
                if number == 601 {
                    staged.plan.promotions[0].bitmap[..2].copy_from_slice(&[1, 1]);
                }
            })
            .is_err()
        );
    }
}

#[test]
fn rewritten_pointer_rows_and_unrelated_future_global_bits_do_not_come_from_staging() {
    assert_eq!(
        forged(|staged| {
            // A valid empty bitmap in another represented span adds no keys,
            // but no immutable request authorizes this extra map pointer.
            staged.plan.promotions[0].after[5..9].copy_from_slice(&521_u32.to_le_bytes());
        })
        .err(),
        Some(EditError::Preservation)
    );
    assert_eq!(
        forged(|staged| {
            // The authorized pages600..605 share this byte.606 is not a request.
            let global = staged
                .plan
                .append
                .map_edits
                .iter_mut()
                .find(|edit| edit.offset == 599 * 4096 + 4 + 600 / 8)
                .unwrap();
            global.after &= !(1 << (606 % 8));
        })
        .err(),
        Some(EditError::Preservation)
    );
}

struct LiteralHeader {
    kind: u8,
    height: u8,
    previous: u32,
    next: u32,
    tail: u32,
    prefix: usize,
}

fn literal_page(header: LiteralHeader, entries: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = vec![0; 4096];
    bytes[..2].copy_from_slice(&[header.kind, 1]);
    bytes[4..8].copy_from_slice(&10_u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&header.previous.to_le_bytes());
    bytes[16..20].copy_from_slice(&header.next.to_le_bytes());
    bytes[20..24].copy_from_slice(&header.tail.to_le_bytes());
    bytes[24..26].copy_from_slice(&(header.prefix as u16).to_le_bytes());
    bytes[26] = header.height;

    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        let omitted = if position == 0 { 0 } else { header.prefix };
        let wire = &entry[omitted..];
        bytes[480 + end..480 + end + wire.len()].copy_from_slice(wire);
        end += wire.len();
        bytes[27 + end / 8] |= 1 << (end % 8);
    }
    bytes[2..4].copy_from_slice(&((4096 - 480 - end) as u16).to_le_bytes());
    bytes
}

#[test]
fn an_existing_nonroot_split_and_a_root_split_share_metadata_allocation_without_gaps() {
    let mut source = model::source(6);
    let mut keys = Vec::new();
    for row in 0..900 {
        let reference = ((model::DATA[row / 200] as u32) << 8) | (row % 200) as u32;
        let mut entry = vec![127, 128, 0, 0, 0];
        entry.extend(reference.to_be_bytes());
        keys.push(entry);
    }
    for (number, entries, previous, next) in [(30, &keys[..200], 0, 31), (31, &keys[200..], 30, 0)]
    {
        let page = literal_page(
            LiteralHeader {
                kind: 4,
                height: 0,
                previous,
                next,
                tail: 0,
                prefix: 5,
            },
            entries,
        );
        model::install(&mut source.bytes, number, &page);
    }
    let mut separator = keys[199].clone();
    separator.extend(30_u32.to_be_bytes());
    let root = literal_page(
        LiteralHeader {
            kind: 3,
            height: 1,
            previous: 0,
            next: 0,
            tail: 31,
            prefix: 0,
        },
        &[separator],
    );
    model::install(&mut source.bytes, 13, &root);
    let (map_start, map_end) = model::row_range(&source.bytes, 14, 0);
    source.bytes[map_start..map_end].copy_from_slice(&model::map(&[13, 30, 31]));

    let before = check_table(
        &mut Cursor::new(&source.bytes),
        CatalogScope::new(1),
        "Items",
        &mut fixed_model::budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        before
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let preview = capture(&source.bytes)
        .preview_ace16_split_fixed_updates(
            "Items",
            "Score",
            &[FixedScalarUpdate::new(
                0,
                Value::Int32(0),
                Value::Int32(i32::MAX),
            )],
            &mut fixed_model::budget(),
            &|| false,
        )
        .unwrap();
    let after = preview.candidate_bytes();
    assert_eq!(after.len(), 605 * 4096);
    for page in [601, 603] {
        assert_eq!(&after[page * 4096..page * 4096 + 4], &[5, 1, 0, 0]);
    }
    for page in [600, 602, 604] {
        assert_eq!(&after[page * 4096..page * 4096 + 2], &[4, 1]);
    }
    for page in [13, 30, 31, 600] {
        assert_ne!(after[601 * 4096 + 4 + page / 8] & (1 << (page % 8)), 0);
    }
    assert_eq!(after[599 * 4096 + 4 + 600 / 8], 0xe0);
    let checked = check_table(
        &mut Cursor::new(after),
        CatalogScope::new(1),
        "Items",
        &mut fixed_model::budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
}
