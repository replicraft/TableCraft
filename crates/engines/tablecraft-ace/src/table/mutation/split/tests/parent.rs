//! Parent-only overflow gives independent after-audit corruption discriminators.
use super::super::{Operation, Staged, finish, stage};
use crate::{Ace16WritePreview, EditError, Int32Update, test_support::split_model as base};
use tablecraft_testkit::access as support;

const PAGE: usize = 4096;
const ENTRY: usize = 480;
const MASK: usize = 27;
const ROOT: usize = 13;
const FIRST: usize = 16;
const CHILDREN: usize = 303;
const EOF: usize = 320;

fn key(row: usize) -> Vec<u8> {
    let mut bytes = vec![127];
    bytes.extend(((row as u32 + 1) ^ 0x8000_0000).to_be_bytes());
    let page = if row < 250 { 12_u32 } else { 15_u32 };
    bytes.extend(((page << 8) | (row % 250) as u32).to_be_bytes());
    bytes
}

fn page(entries: &[Vec<u8>], node: bool) -> Vec<u8> {
    let mut bytes = vec![0xa7; PAGE];
    bytes[..ENTRY].fill(0);
    bytes[..2].copy_from_slice(&[if node { 3 } else { 4 }, 1]);
    support::put32(&mut bytes, 4, 10);
    support::put16(&mut bytes, 24, 3);
    bytes[26] = u8::from(node);
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        let shared = if position == 0 { 0 } else { 3 };
        let stored = &entry[shared..];
        bytes[ENTRY + end..ENTRY + end + stored.len()].copy_from_slice(stored);
        end += stored.len();
        bytes[MASK + end / 8] |= 1 << (end % 8);
    }
    support::put16(&mut bytes, 2, (PAGE - ENTRY - end) as u16);
    bytes
}

fn source() -> Vec<u8> {
    let mut bytes = base::source(6, false, true);
    bytes.resize(EOF * PAGE, 0);
    let mut bounds = Vec::new();
    for position in 0..CHILDREN {
        let number = FIRST + position;
        let values = if position + 1 == CHILDREN {
            (position..500).map(key).collect::<Vec<_>>()
        } else {
            vec![key(position)]
        };
        let mut leaf = page(&values, false);
        support::put32(
            &mut leaf,
            12,
            if position == 0 { 0 } else { number as u32 - 1 },
        );
        support::put32(
            &mut leaf,
            16,
            if position + 1 == CHILDREN {
                0
            } else {
                number as u32 + 1
            },
        );
        bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(&leaf);
        if position + 1 < CHILDREN {
            let mut bound = values.last().unwrap().clone();
            bound.extend((number as u32).to_be_bytes());
            bounds.push(bound);
        }
    }
    let mut root = page(&bounds, true);
    support::put32(&mut root, 20, (FIRST + CHILDREN - 1) as u32);
    bytes[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&root);
    let mut owned = vec![0; 69];
    for number in std::iter::once(ROOT).chain(FIRST..FIRST + CHILDREN) {
        owned[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[14 * PAGE..15 * PAGE].copy_from_slice(&support::data_page(0, &[(0, owned)]));
    let mut global = vec![0; 69];
    for number in EOF..512 {
        global[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[PAGE..2 * PAGE].copy_from_slice(&support::data_page(1, &[(0, global)]));
    bytes
}

fn forged(change: impl FnOnce(&mut Staged)) -> Result<Ace16WritePreview, EditError> {
    let before = source();
    base::assert_checked(&before);
    let operation = Operation {
        table: "Items",
        column: "Score",
        requests: &[Int32Update::new(0, 1, i32::MIN)],
    };
    let image = base::capture(&before);
    let mut staged = stage(&image, &operation, &mut base::budget(), &|| false)?;
    // No leaf overflows here. Only the old root splits, allocating right then left.
    assert_eq!(staged.plan.added.len(), 2);
    assert_eq!(staged.plan.added[0].number as usize, EOF);
    assert_eq!(staged.plan.added[1].number as usize, EOF + 1);
    let healthy = finish::finish(
        base::capture(&before),
        &operation,
        &staged,
        &mut base::budget(),
        &|| false,
    )?;
    base::assert_checked(healthy.candidate_bytes());
    assert_eq!(healthy.candidate_bytes()[ROOT * PAGE + 26], 2);
    change(&mut staged);
    finish::finish(image, &operation, &staged, &mut base::budget(), &|| false)
}

#[test]
fn parent_root_bounds_height_tail_and_original_residual_have_no_supplied_authority() {
    for offset in [488, 26, 20, PAGE - 1] {
        assert_eq!(
            forged(|staged| {
                let root = staged
                    .plan
                    .patches
                    .iter_mut()
                    .find(|patch| patch.offset == ROOT * PAGE)
                    .unwrap();
                root.after[offset] ^= 1;
            })
            .err(),
            Some(EditError::Preservation)
        );
    }
}

#[test]
fn fresh_internal_links_padding_and_unrequested_map_bits_are_rejected() {
    for offset in [12, 16, 20, PAGE - 1] {
        assert_eq!(
            forged(|staged| {
                staged.plan.added[0].bytes[offset] ^= 1;
            })
            .err(),
            Some(EditError::Preservation)
        );
    }
    assert_eq!(
        forged(|staged| {
            let global = staged
                .plan
                .append
                .map_edits
                .iter_mut()
                .find(|edit| edit.offset / PAGE == 1)
                .unwrap();
            global.after &= !4;
        })
        .err(),
        Some(EditError::Preservation)
    );
}
