//! Healthy checked candidates distinguish authority failures from corrupt input.
mod ignored;
mod long_values;
mod lval_growth;
mod multiple;

use super::super::{RowInsert, Staged, compose, finish, preflight, prepare, tests::model};
use super::prove;
use crate::{
    EditError, IndexCheckStatus, NativeImage, check_table, check_table_constraints,
    table::{indexes::mutation::IndexDataPatch, mutation::gates},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;
const PRIMARY: usize = 13 * PAGE;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
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

fn requests(value: i32, length: usize) -> Vec<RowInsert> {
    vec![RowInsert::new(vec![
        Value::Int32(value),
        Value::Binary(vec![0x72; length]),
    ])]
}

fn staged(before: &[u8], requests: &[RowInsert]) -> (NativeImage, Staged) {
    let image = capture(before);
    let checked = preflight(&image, "Items", requests, &mut budget(), &|| false).unwrap();
    let staged = prepare(&image, &checked, requests, &mut budget(), &|| false).unwrap();
    (image, staged)
}

fn checked(bytes: &[u8], rows: usize) {
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), rows);
    assert!(
        checked
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
    gates::qualified(&constraints).unwrap();
}

fn healthy(before: &[u8], requests: &[RowInsert]) -> Vec<u8> {
    checked(before, 3);
    let (image, staged) = staged(before, requests);
    let candidate = finish(image, "Items", requests, &staged, &mut budget(), &|| false).unwrap();
    assert_eq!(candidate.changed_rows(), requests.len());
    checked(candidate.candidate_bytes(), 3 + requests.len());
    candidate.candidate_bytes().to_vec()
}

#[test]
fn immutable_requests_and_original_bytes_reject_correlated_valid_insertions() {
    let before = model::source(5, true, false, None);
    let requested = requests(10, 3);
    let after = healthy(&before, &requested);
    let changed = requests(11, 3);
    let (image, forged) = staged(&before, &changed);
    let forged_after = compose::compose(&image, &forged, &mut budget(), &|| false).unwrap();
    checked(&forged_after, 4);
    assert_ne!(forged_after, after);
    assert_eq!(
        prove(
            &image,
            "Items",
            &requested,
            &forged,
            &forged_after,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation),
    );
    assert_eq!(
        prove(
            &image,
            "Items",
            &[],
            &forged,
            &forged_after,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation),
    );

    let (original, original_stage) = staged(&before, &requested);
    let mut changed_source = before.clone();
    changed_source[9 * PAGE + 127] ^= 1;
    checked(&changed_source, 3);
    let changed_source = capture(&changed_source);
    assert_eq!(
        prove(
            &original,
            "Items",
            &requested,
            &original_stage,
            &after,
            &mut budget(),
            &|| false
        ),
        Ok(()),
    );
    assert_eq!(
        prove(
            &changed_source,
            "Items",
            &requested,
            &original_stage,
            &after,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation),
    );
}

#[test]
fn checked_counter_and_future_bit_changes_cannot_extend_request_authority() {
    let before = model::source(6, true, true, None);
    let requested = requests(10, 255);
    let after = healthy(&before, &requested);
    for mutation in 0..5 {
        let (image, mut forged) = staged(&before, &requested);
        match mutation {
            0 => {
                let value = u32::from_le_bytes(forged.counters[1].after);
                forged.counters[1].after = (value + 1).to_le_bytes();
            }
            1 => {
                let byte = forged
                    .data
                    .append
                    .map_edits
                    .iter_mut()
                    .find(|byte| byte.offset / PAGE == 1)
                    .unwrap();
                assert_ne!(byte.before & 0x80, 0);
                byte.after &= !0x80;
            }
            2 => {
                forged.data.rows[0].range.end -= 1;
            }
            3 => {
                let page = forged
                    .data
                    .pages
                    .iter_mut()
                    .find(|page| page.before.is_empty())
                    .unwrap();
                page.after[..2].copy_from_slice(&[4, 1]);
            }
            4 => {
                let length = forged.data.append.length;
                let number = forged.data.append.next().unwrap();
                forged
                    .data
                    .append
                    .reserve_data(number, &mut budget(), &|| false)
                    .unwrap();
                forged.data.append.length = length;
            }
            _ => unreachable!(),
        }
        let forged_after = compose::compose(&image, &forged, &mut budget(), &|| false).unwrap();
        if mutation != 3 {
            checked(&forged_after, 4);
        }
        if matches!(mutation, 2 | 4) {
            assert_eq!(forged_after, after);
        }
        assert_eq!(
            prove(
                &image,
                "Items",
                &requested,
                &forged,
                &forged_after,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation),
            "mutation {mutation}",
        );
    }
}

#[test]
fn structurally_valid_primary_recompression_and_slack_edits_still_refuse() {
    let before = model::source(5, true, false, None);
    let requested = requests(10, 3);
    let after = healthy(&before, &requested);
    for mutation in 0..2 {
        let (image, mut forged) = staged(&before, &requested);
        if mutation == 0 {
            // All four complete wires share marker 127. Prefix 1 is valid wire
            // framing but violates this retained leaf's immutable seed 0.
            assert_eq!(&after[PRIMARY + 24..PRIMARY + 26], &[0, 0]);
            let mut root = before[PRIMARY..PRIMARY + PAGE].to_vec();
            root[27..480].fill(0);
            let mut end = 0;
            for (position, (value, reference)) in forged.indexes[0].expected().enumerate() {
                let mut wire = value.to_vec();
                wire.extend(reference.to_be_bytes());
                let suffix = &wire[usize::from(position != 0)..];
                root[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
                end += suffix.len();
                root[27 + end / 8] |= 1 << (end % 8);
            }
            root[2..4].copy_from_slice(&((3616 - end) as u16).to_le_bytes());
            root[24..26].copy_from_slice(&1_u16.to_le_bytes());
            forged.indexes[0].patches.clear();
            for range in [2..4, 24..26, 27..480 + end] {
                forged.indexes[0].patches.push(IndexDataPatch {
                    offset: PRIMARY + range.start,
                    before: before[PRIMARY + range.start..PRIMARY + range.end].to_vec(),
                    after: root[range].to_vec(),
                });
            }
        } else {
            let offset = PRIMARY + PAGE - 1;
            forged.indexes[0].patches.push(IndexDataPatch {
                offset,
                before: vec![before[offset]],
                after: vec![before[offset] ^ 1],
            });
        }
        let forged_after = compose::compose(&image, &forged, &mut budget(), &|| false).unwrap();
        checked(&forged_after, 4);
        assert_ne!(forged_after, after);
        assert_eq!(
            prove(
                &image,
                "Items",
                &requested,
                &forged,
                &forged_after,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation),
            "mutation {mutation}",
        );
    }
}
