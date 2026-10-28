//! Preserved fitted source controls and recursive raw-proof adversaries.
mod preservation;
use super::*;
use crate::{
    IndexCheckStatus, RowInsert, check_table, check_table_constraints,
    table::{
        indexes::mutation::IndexDataPatch,
        mutation::insert::{audit, compose, finish, preflight, prepare, tests::model},
    },
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const PAGE: usize = 4096;
const ROOT: usize = 13;
const LEFT: usize = 15;
const RIGHT: usize = 16;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn wire(value: i32, reference: u32) -> [u8; 9] {
    let mut bytes = [0; 9];
    bytes[0] = 127;
    bytes[1..5].copy_from_slice(&((value as u32) ^ 0x80000000).to_be_bytes());
    bytes[5..].copy_from_slice(&reference.to_be_bytes());
    bytes
}

fn inline(members: impl IntoIterator<Item = usize>) -> Vec<u8> {
    let mut row = vec![0; 69];
    for member in members {
        row[5 + member / 8] |= 1 << (member % 8);
    }
    row
}

fn install(bytes: &mut [u8], number: usize, page: &[u8]) {
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

fn leaf(entries: &[[u8; 9]], prefix: usize, previous: usize, next: usize, residue: u8) -> Vec<u8> {
    let mut page = vec![residue; PAGE];
    page[..480].fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    support::put32(&mut page, 4, 10);
    support::put32(&mut page, 12, previous as u32);
    support::put32(&mut page, 16, next as u32);
    support::put16(&mut page, 24, prefix as u16);

    let mut active = 0;
    for (position, entry) in entries.iter().enumerate() {
        let suffix = &entry[if position == 0 { 0 } else { prefix }..];
        page[480 + active..480 + active + suffix.len()].copy_from_slice(suffix);
        active += suffix.len();
        page[27 + active / 8] |= 1 << (active % 8);
    }
    support::put16(&mut page, 2, (3616 - active) as u16);
    page
}

fn source(code: u8) -> Vec<u8> {
    let mut bytes = model::source(code, true, false, None);
    bytes.resize(17 * PAGE, 0);
    let left = leaf(
        &[wire(1, 12 << 8), wire(2, (12 << 8) | 1)],
        4,
        0,
        RIGHT,
        0xb6,
    );
    let right = leaf(&[wire(3, (12 << 8) | 2)], 4, LEFT, 0, 0xc7);
    install(&mut bytes, LEFT, &left);
    install(&mut bytes, RIGHT, &right);

    let mut root = bytes[ROOT * PAGE..(ROOT + 1) * PAGE].to_vec();
    root[..480].fill(0);
    root[..2].copy_from_slice(&[3, 1]);
    support::put16(&mut root, 2, 3603);
    support::put32(&mut root, 4, 10);
    support::put32(&mut root, 20, RIGHT as u32);
    root[26] = 1;
    root[480..489].copy_from_slice(&wire(2, (12 << 8) | 1));
    root[489..493].copy_from_slice(&(LEFT as u32).to_be_bytes());
    root[28] |= 1 << 5;
    install(&mut bytes, ROOT, &root);
    install(
        &mut bytes,
        14,
        &support::data_page(0, &[(0, inline([ROOT, LEFT, RIGHT]))]),
    );
    install(
        &mut bytes,
        1,
        &support::data_page(1, &[(0, inline(17..512))]),
    );
    bytes
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn checked(bytes: &[u8], rows: usize) {
    let structural = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(structural.rows(), rows);
    assert_eq!(structural.indexes().len(), 2);
    assert!(
        structural
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );

    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), rows);
}

fn request_rows(values: &[i32]) -> Vec<RowInsert> {
    values
        .iter()
        .map(|value| RowInsert::new(vec![Value::Int32(*value), Value::Null]))
        .collect()
}

#[test]
fn original_inclusive_bound_routes_left_right_bulk_and_empty_requests() {
    for code in [5, 6] {
        for values in [&[][..], &[0][..], &[4][..], &[0, 4][..]] {
            let before = source(code);
            checked(&before, 3);
            let image = capture(&before);
            let requests = request_rows(values);
            let checked = preflight(&image, "Items", &requests, &mut budget(), &|| false).unwrap();
            let staged = prepare(&image, &checked, &requests, &mut budget(), &|| false).unwrap();
            assert!(staged.indexes[0].added.is_empty());
            assert!(staged.indexes[0].promotions.is_empty());
            assert!(staged.indexes[0].rebases.is_empty());

            let candidate =
                finish(image, "Items", &requests, &staged, &mut budget(), &|| false).unwrap();
            let after = candidate.candidate_bytes();
            self::checked(after, 3 + values.len());
            assert_eq!(after.len(), before.len());
            assert_eq!(
                &after[ROOT * PAGE..(ROOT + 1) * PAGE],
                &before[ROOT * PAGE..(ROOT + 1) * PAGE]
            );
            for (number, changed) in [(LEFT, values.contains(&0)), (RIGHT, values.contains(&4))] {
                if !changed {
                    assert_eq!(
                        &after[number * PAGE..(number + 1) * PAGE],
                        &before[number * PAGE..(number + 1) * PAGE]
                    );
                }
                assert_eq!(
                    &after[number * PAGE + 512..(number + 1) * PAGE],
                    &before[number * PAGE + 512..(number + 1) * PAGE]
                );
            }
            for number in [1, 11, 14] {
                assert_eq!(
                    &after[number * PAGE..(number + 1) * PAGE],
                    &before[number * PAGE..(number + 1) * PAGE]
                );
            }
        }
    }
}

#[test]
fn semantically_checked_root_and_untouched_leaf_residue_remain_protected() {
    let before = source(5);
    checked(&before, 3);
    let image = capture(&before);
    let requests = request_rows(&[4]);
    let checked = preflight(&image, "Items", &requests, &mut budget(), &|| false).unwrap();
    let mut staged = prepare(&image, &checked, &requests, &mut budget(), &|| false).unwrap();
    let healthy = compose::compose(&image, &staged, &mut budget(), &|| false).unwrap();
    audit::prove(
        &image,
        "Items",
        &requests,
        &staged,
        &healthy,
        &mut budget(),
        &|| false,
    )
    .unwrap();

    for number in [ROOT, LEFT] {
        let original = before[number * PAGE..(number + 1) * PAGE].to_vec();
        let mut forged = original.clone();
        forged[3000] ^= 1;
        staged.indexes[0].patches.push(IndexDataPatch {
            offset: number * PAGE,
            before: original,
            after: forged,
        });
        let after = compose::compose(&image, &staged, &mut budget(), &|| false).unwrap();
        self::checked(&after, 4);
        assert_eq!(
            audit::prove(
                &image,
                "Items",
                &requests,
                &staged,
                &after,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        );
        staged.indexes[0].patches.pop();
    }

    let changed = request_rows(&[0]);
    assert_eq!(
        audit::prove(
            &image,
            "Items",
            &changed,
            &staged,
            &healthy,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    );
}
