//! Immutable tuple requests certify existing-fit wire framing independently.
use super::super::{Staged, finish, fixed::FixedOperation, preflight, stage_checked};
use crate::{
    EditError, FixedScalarUpdate, IndexCheckStatus, check_table, check_table_constraints,
    read_table, read_table_index,
    table::{indexes::IndexDataPatch, mutation::gates},
    test_support::{
        split_model::{budget, capture},
        tuples::{decimal_fixture, mixed_fixture, numeric_fixture},
    },
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Guid, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const PAGE: usize = 4096;
const ROOT: usize = 14;
const ROWS: usize = 200;
const ENTRY_START: usize = 480;
const BITMAP_START: usize = 27;
const FREE_OFFSET: usize = 2;
const PREFIX_OFFSET: usize = 24;

fn entry(slot: usize, score: i32) -> Vec<u8> {
    let mut wire = vec![0x7f, 0x80, 0, 0, 0, 0x7f];
    wire.extend(((score as u32) ^ 0x8000_0000).to_be_bytes());
    wire.extend(((12_u32 << 8) | slot as u32).to_be_bytes());
    wire
}

fn repack(original: &[u8], entries: &[Vec<u8>], prefix: usize) -> Vec<u8> {
    let mut after = original.to_vec();
    after[BITMAP_START..ENTRY_START].fill(0);
    let mut length = 0;
    for (position, wire) in entries.iter().enumerate() {
        let shared = if position == 0 { 0 } else { prefix };
        assert_eq!(wire[..shared], entries[0][..shared]);
        let suffix = &wire[shared..];
        after[ENTRY_START + length..ENTRY_START + length + suffix.len()].copy_from_slice(suffix);
        length += suffix.len();
        after[BITMAP_START + length / 8] |= 1 << (length % 8);
    }
    after[FREE_OFFSET..FREE_OFFSET + 2]
        .copy_from_slice(&((PAGE - ENTRY_START - length) as u16).to_le_bytes());
    after[PREFIX_OFFSET..PREFIX_OFFSET + 2].copy_from_slice(&(prefix as u16).to_le_bytes());
    after
}

fn compose(before: &[u8], staged: &Staged<4>) -> Vec<u8> {
    assert!(staged.plan.added.is_empty());
    assert!(staged.plan.append.map_edits.is_empty());
    assert_eq!(staged.plan.append.length, before.len());
    let mut after = before.to_vec();
    for patch in &staged.patches {
        after[patch.offset..patch.offset + patch.after.len()].copy_from_slice(&patch.after);
    }
    for patch in &staged.plan.patches {
        after[patch.offset..patch.offset + patch.after.len()].copy_from_slice(&patch.after);
    }
    after
}

fn checked(bytes: &[u8], changed: bool) {
    let mut expected = vec![vec![Value::Int32(0), Value::Int32(0)]; ROWS];
    if changed {
        expected[0][1] = Value::Int32(1);
    }
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows(), expected);
    let report = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.indexes().len(), 1);
    assert_eq!(report.indexes()[0].status(), IndexCheckStatus::Checked);
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    gates::qualified(&constraints).unwrap();
    let ordered = read_table_index(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    if changed {
        expected.rotate_left(1);
    }
    assert_eq!(ordered.rows(), expected);
}

#[test]
fn valid_composite_prefix_cannot_replace_original_seed_authority() {
    let rows = vec![vec![Some(0), Some(0)]; ROWS];
    let before = numeric_fixture::database(6, &[4, 4], &[(0, false), (1, false)], &rows, 13);
    checked(&before, false);
    let source_root = &before[ROOT * PAGE..(ROOT + 1) * PAGE];
    assert_eq!(&source_root[PREFIX_OFFSET..PREFIX_OFFSET + 2], &[13, 0]);
    let original_entries = (0..ROWS).map(|slot| entry(slot, 0)).collect::<Vec<_>>();
    assert_eq!(repack(source_root, &original_entries, 13), source_root);
    let entries = (1..ROWS)
        .map(|slot| entry(slot, 0))
        .chain(std::iter::once(entry(0, 1)))
        .collect::<Vec<_>>();
    assert_eq!(entries[0].len(), 14);
    assert_eq!(entries[0][..9], entries[ROWS - 1][..9]);
    assert_ne!(entries[0][9], entries[ROWS - 1][9]);
    assert_eq!(14 + (ROWS - 1) * (14 - 9), 1009);
    assert_eq!(14 + (ROWS - 1) * (14 - 8), 1208);

    let requests = [FixedScalarUpdate::new(0, Value::Int32(0), Value::Int32(1))];
    let operation = FixedOperation {
        table: "Items",
        column: "Score",
        requests: &requests,
    };
    let image = capture(&before);
    let structural = preflight(&image, &operation, &mut budget(), &|| false).unwrap();
    let mut staged =
        stage_checked::<4>(&image, &operation, structural, &mut budget(), &|| false).unwrap();
    let healthy = finish::bound(
        capture(&before),
        &operation,
        &staged,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    checked(healthy.candidate_bytes(), true);
    let healthy_root = &healthy.candidate_bytes()[ROOT * PAGE..(ROOT + 1) * PAGE];
    assert_eq!(healthy_root, repack(source_root, &entries, 9));

    staged
        .plan
        .patches
        .retain(|patch| patch.offset / PAGE != ROOT);
    staged.plan.patches.push(IndexDataPatch {
        offset: ROOT * PAGE,
        before: source_root.to_vec(),
        after: repack(source_root, &entries, 8),
    });
    checked(&compose(&before, &staged), true);
    assert_eq!(
        finish::bound(image, &operation, &staged, &mut budget(), &|| false).err(),
        Some(EditError::Preservation),
    );
}

fn families(code: u8, descending: bool) -> Vec<Vec<u8>> {
    use decimal_fixture::Cell as DecimalCell;
    use mixed_fixture::Cell as MixedCell;
    let order = [(0, descending), (1, !descending)];
    vec![
        numeric_fixture::database(code, &[4, 4], &order, &vec![vec![Some(0), Some(1)]; 3], 0),
        mixed_fixture::database(
            code,
            &[4, 15],
            &order,
            &vec![vec![Some(MixedCell::Number(0)), Some(MixedCell::Guid([1; 16]))]; 3],
            0,
        ),
        decimal_fixture::database(
            code,
            &[4, 16],
            &order,
            &vec![vec![Some(DecimalCell::Number(0)), Some(DecimalCell::Decimal(1))]; 3],
            0,
        ),
    ]
}

fn assert_complete(bytes: &[u8]) -> Vec<Vec<Value>> {
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    gates::qualified(
        &check_table_constraints(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    )
    .unwrap();
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .rows()
    .to_vec()
}

#[test]
fn numeric_guid_and_decimal_companions_keep_exact_values_and_full_row_order() {
    for code in [5, 6] {
        for descending in [false, true] {
            for before in families(code, descending) {
                let mut expected = assert_complete(&before);
                let original = before.clone();
                let requests = [FixedScalarUpdate::new(0, Value::Int32(0), Value::Int32(1))];
                let result = capture(&before)
                    .preview_ace16_split_fixed_updates(
                        "Items",
                        "Id",
                        &requests,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                expected[0][0] = Value::Int32(1);
                assert_eq!(assert_complete(result.candidate_bytes()), expected);
                assert_eq!(result.candidate_bytes().len(), before.len());
                let index = read_table_index(
                    &mut Cursor::new(result.candidate_bytes()),
                    CatalogScope::new(1),
                    "Items",
                    "Compound",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                if !descending {
                    expected.rotate_left(1);
                }
                assert_eq!(index.rows(), expected);
                assert_eq!(before, original);
            }
        }
    }
    assert_eq!(
        Value::Guid(Guid::from_bytes([1; 16])),
        assert_complete(&families(6, false)[1])[0][1]
    );
}

fn index_page(height: u8, entries: &[Vec<u8>], previous: u32, next: u32, tail: u32) -> Vec<u8> {
    let mut bytes = vec![0xa7; PAGE];
    bytes[..ENTRY_START].fill(0);
    bytes[0] = if height == 0 { 4 } else { 3 };
    bytes[1] = 1;
    bytes[26] = height;
    bytes[4..8].copy_from_slice(&10_u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&previous.to_le_bytes());
    bytes[16..20].copy_from_slice(&next.to_le_bytes());
    bytes[20..24].copy_from_slice(&tail.to_le_bytes());
    repack(&bytes, entries, 0)
}

#[test]
fn original_bounded_and_tail_nodes_keep_child_maxima_links_and_residue() {
    for bounded in [false, true] {
        let mut before = numeric_fixture::database(
            6,
            &[4, 4],
            &[(0, false), (1, false)],
            &vec![vec![Some(0), Some(0)]; 3],
            0,
        );
        before.resize(17 * PAGE, 0);
        let wires = (0..3).map(|row| entry(row, 0)).collect::<Vec<_>>();
        let mut bound = wires[1].clone();
        bound.extend(15_u32.to_be_bytes());
        let mut bounds = vec![bound];
        if bounded {
            let mut last = wires[2].clone();
            last.extend(16_u32.to_be_bytes());
            bounds.push(last);
        }
        for (number, raw) in [
            (
                14,
                index_page(1, &bounds, 0, 0, if bounded { 0 } else { 16 }),
            ),
            (15, index_page(0, &wires[..2], 0, 16, 0)),
            (16, index_page(0, &wires[2..], 15, 0, 0)),
        ] {
            before[number * PAGE..(number + 1) * PAGE].copy_from_slice(&raw);
        }
        before[13 * PAGE..14 * PAGE].copy_from_slice(&support::data_page(
            1,
            &[(0, vec![0, 0, 0, 0, 0, 0, 0xc0, 1])],
        ));
        let mut expected = assert_complete(&before);
        let requests = [FixedScalarUpdate::new(0, Value::Int32(0), Value::Int32(1))];
        let result = capture(&before)
            .preview_ace16_split_fixed_updates("Items", "Score", &requests, &mut budget(), &|| {
                false
            })
            .unwrap();
        expected[0][1] = Value::Int32(1);
        assert_eq!(assert_complete(result.candidate_bytes()), expected);
        assert_eq!(result.candidate_bytes().len(), before.len());
        for number in [14, 15, 16] {
            assert_eq!(
                &result.candidate_bytes()[(number + 1) * PAGE - 32..(number + 1) * PAGE],
                &[0xa7; 32]
            );
        }
    }
}

#[test]
fn tuple_fitting_certification_preserves_conflicts_limits_cancellation_and_retry() {
    for before in families(6, false) {
        let image = capture(&before);
        let request = FixedScalarUpdate::new(0, Value::Int32(0), Value::Int32(1));
        let stale = [FixedScalarUpdate::new(0, Value::Int32(9), Value::Int32(1))];
        assert_eq!(
            capture(&before)
                .preview_ace16_split_fixed_updates("Items", "Id", &stale, &mut budget(), &|| false)
                .err(),
            Some(EditError::Conflict)
        );
        assert_eq!(
            capture(&before)
                .preview_ace16_split_fixed_updates(
                    "Items",
                    "Id",
                    &[
                        FixedScalarUpdate::new(0, Value::Int32(0), Value::Int32(1)),
                        FixedScalarUpdate::new(0, Value::Int32(0), Value::Int32(1)),
                    ],
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(EditError::Conflict)
        );
        for mut limited in [
            ResourceBudget::new(0, 100_000, 100_000_000),
            ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
            ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
        ] {
            assert!(
                capture(&before)
                    .preview_ace16_split_fixed_updates(
                        "Items",
                        "Id",
                        std::slice::from_ref(&request),
                        &mut limited,
                        &|| false
                    )
                    .is_err()
            );
        }
        assert!(
            capture(&before)
                .preview_ace16_split_fixed_updates(
                    "Items",
                    "Id",
                    std::slice::from_ref(&request),
                    &mut budget(),
                    &|| true
                )
                .is_err()
        );
        let result = capture(&before)
            .preview_ace16_split_fixed_updates(
                "Items",
                "Id",
                std::slice::from_ref(&request),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(
            assert_complete(result.candidate_bytes())[0][0],
            Value::Int32(1)
        );
        let calls = Cell::new(0_usize);
        let measured = capture(&before)
            .preview_ace16_split_fixed_updates(
                "Items",
                "Id",
                std::slice::from_ref(&request),
                &mut budget(),
                &|| {
                    calls.set(calls.get() + 1);
                    false
                },
            )
            .unwrap();
        let checks = calls.get();
        assert!(checks > 10);
        calls.set(0);
        assert!(
            capture(&before)
                .preview_ace16_split_fixed_updates(
                    "Items",
                    "Id",
                    std::slice::from_ref(&request),
                    &mut budget(),
                    &|| {
                        calls.set(calls.get() + 1);
                        calls.get() >= checks - 1
                    },
                )
                .is_err()
        );
        let retry = capture(&before)
            .preview_ace16_split_fixed_updates(
                "Items",
                "Id",
                std::slice::from_ref(&request),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(retry.candidate_bytes(), measured.candidate_bytes());
        assert_eq!(image.as_bytes(), before);
    }
}
