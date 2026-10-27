//! Structurally valid altered images cannot authorize a different split policy.
use super::super::{Staged, finish, fixed::FixedOperation, preflight, stage_checked};
use crate::{
    EditError, check_table_constraints,
    table::mutation::gates,
    test_support::fixed_split::model::{self, Kind, PAGE, ROOTS, budget, request, source},
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

const PREVIOUS_OFFSET: usize = 12;
const NEXT_OFFSET: usize = 16;
const PREFIX_OFFSET: usize = 24;
const ENTRY_START: usize = 480;
const BITMAP_START: usize = 27;
const FREE_OFFSET: usize = 2;
const KEY_ROW_BYTES: usize = 13;
const CHILD_BYTES: usize = size_of::<u32>();
const NODE_ENTRY_BYTES: usize = KEY_ROW_BYTES + CHILD_BYTES;

fn compose(before: &[u8], staged: &Staged<8>) -> Vec<u8> {
    let mut after = before.to_vec();
    after.resize(staged.plan.length, 0);
    for patch in &staged.patches {
        after[patch.offset..patch.offset + patch.after.len()].copy_from_slice(&patch.after);
    }
    for patch in &staged.plan.patches {
        after[patch.offset..patch.offset + patch.after.len()].copy_from_slice(&patch.after);
    }
    for edit in &staged.plan.map_edits {
        after[edit.offset] = edit.after;
    }
    for page in &staged.plan.added {
        let start = page.number as usize * PAGE;
        after[start..start + PAGE].copy_from_slice(&page.bytes);
    }
    after
}

fn checked_values(after: Vec<u8>, count: usize, value: i64) {
    let mut model = source(6, Kind::Int64, vec![count], false);
    model.bytes = after;
    model.values[0] = Some(value);
    model::check(&model);
    let constraints = check_table_constraints(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    gates::qualified(&constraints).unwrap();
}

fn checked(after: Vec<u8>) {
    checked_values(after, 900, i64::MAX);
}

fn forged(change: impl FnOnce(&mut Staged<8>, usize)) -> EditError {
    let model = source(6, Kind::Int64, vec![900], false);
    model::check(&model);
    let before = model.bytes.clone();
    let requests = [request(&model)];
    let operation = FixedOperation {
        table: "Items",
        column: "Score",
        requests: &requests,
    };
    let image = crate::test_support::split_model::capture(&before);
    let structural = preflight(&image, &operation, &mut budget(), &|| false).unwrap();
    let mut staged =
        stage_checked::<8>(&image, &operation, structural, &mut budget(), &|| false).unwrap();
    let healthy = finish::bound(
        crate::test_support::split_model::capture(&before),
        &operation,
        &staged,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    checked(healthy.candidate_bytes().to_vec());
    change(&mut staged, before.len() / PAGE);
    // Both the ordinary reader/checker and complete constraints still accept
    // each forged final image. Only independent operation certification refuses.
    checked(compose(&before, &staged));
    finish::bound(image, &operation, &staged, &mut budget(), &|| false)
        .err()
        .unwrap()
}

#[test]
fn fresh_pages_split_again_have_no_transient_padding_authority() {
    for relative in [0, 4] {
        assert_eq!(
            forged(|staged, eof| {
                let page = staged
                    .plan
                    .added
                    .iter_mut()
                    .find(|page| page.number as usize == eof + relative)
                    .unwrap();
                assert_eq!(page.bytes[PAGE - 1], 0);
                page.bytes[PAGE - 1] = 0x6d;
            }),
            EditError::Preservation
        );
    }
}

#[test]
fn an_unused_future_global_bit_is_not_a_split_event() {
    assert_eq!(
        forged(|staged, eof| {
            assert_eq!(eof, 24);
            let edit = staged
                .plan
                .map_edits
                .iter_mut()
                .find(|edit| edit.offset / PAGE == 1)
                .unwrap();
            assert_eq!(edit.after & (1 << 6), 1 << 6);
            edit.after &= !(1 << 6);
        }),
        EditError::Preservation
    );
}

#[test]
fn valid_correlated_leaf_page_permutation_does_not_replace_declared_schedule() {
    assert_eq!(
        forged(|staged, eof| {
            let first = staged
                .plan
                .added
                .iter()
                .position(|page| page.number as usize == eof + 1)
                .unwrap();
            let second = staged
                .plan
                .added
                .iter()
                .position(|page| page.number as usize == eof + 2)
                .unwrap();
            let first_body = staged.plan.added[first].bytes.clone();
            let second_body = staged.plan.added[second].bytes.clone();
            staged.plan.added[first].bytes = second_body;
            staged.plan.added[second].bytes = first_body;
            let first_id = (eof + 1) as u32;
            let second_id = (eof + 2) as u32;
            for page in &mut staged.plan.added {
                let (previous, next) = match page.number as usize - eof {
                    0 => (first_id, 0),
                    1 => (second_id, eof as u32),
                    2 => (0, first_id),
                    _ => continue,
                };
                page.bytes[PREVIOUS_OFFSET..PREVIOUS_OFFSET + CHILD_BYTES]
                    .copy_from_slice(&previous.to_le_bytes());
                page.bytes[NEXT_OFFSET..NEXT_OFFSET + CHILD_BYTES]
                    .copy_from_slice(&next.to_le_bytes());
            }
            let root = staged
                .plan
                .patches
                .iter_mut()
                .find(|patch| patch.offset == ROOTS[0] * PAGE)
                .unwrap();
            assert_eq!(
                &root.after[PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()],
                &[0, 0]
            );
            let first_child = ENTRY_START + KEY_ROW_BYTES;
            let second_child = first_child + NODE_ENTRY_BYTES;
            root.after[first_child..first_child + CHILD_BYTES]
                .copy_from_slice(&second_id.to_be_bytes());
            root.after[second_child..second_child + CHILD_BYTES]
                .copy_from_slice(&first_id.to_be_bytes());
        }),
        EditError::Preservation
    );
}

#[test]
fn correlated_value_and_leaf_changes_cannot_authorize_unnecessary_growth() {
    let model = source(6, Kind::Int64, vec![400], false);
    let before = model.bytes.clone();
    let maximum = [request(&model)];
    let maximum_operation = FixedOperation {
        table: "Items",
        column: "Score",
        requests: &maximum,
    };
    let image = crate::test_support::split_model::capture(&before);
    let structural = preflight(&image, &maximum_operation, &mut budget(), &|| false).unwrap();
    let mut staged = stage_checked::<8>(
        &image,
        &maximum_operation,
        structural,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let healthy = finish::bound(
        crate::test_support::split_model::capture(&before),
        &maximum_operation,
        &staged,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    checked_values(healthy.candidate_bytes().to_vec(), 400, i64::MAX);
    // Unlike MAX, exact value1 fits the original400-entry page. Correlate every
    // visible field/key byte to1 while keeping the unnecessary two-leaf topology.
    let fitting = [crate::FixedScalarUpdate::new(
        0,
        Value::Int64(0),
        Value::Int64(1),
    )];
    let operation = FixedOperation {
        table: "Items",
        column: "Score",
        requests: &fitting,
    };
    staged.patches[0].after = 1_i64.to_le_bytes();
    let mut replaced = 0;
    for page in &mut staged.plan.added {
        let descending = page.number as usize >= model.eof + 2;
        let old = model::wire(Kind::Int64, 0, Some(i64::MAX), descending);
        let new = model::wire(Kind::Int64, 0, Some(1), descending);
        assert_eq!(
            &page.bytes[PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()],
            &[0, 0]
        );
        let matches = page.bytes[ENTRY_START..]
            .windows(KEY_ROW_BYTES)
            .enumerate()
            .filter_map(|(offset, bytes)| (bytes == old).then_some(ENTRY_START + offset))
            .collect::<Vec<_>>();
        assert!(matches.len() <= 1);
        if let Some(&offset) = matches.first() {
            page.bytes[offset..offset + KEY_ROW_BYTES].copy_from_slice(&new);
            replaced += 1;
        }
    }
    assert_eq!(replaced, 2);
    checked_values(compose(&before, &staged), 400, 1);
    assert_eq!(
        finish::bound(image, &operation, &staged, &mut budget(), &|| false).err(),
        Some(EditError::Preservation)
    );
}

// Independent packets retain semantic keys while violating prefix policy.
fn repack_with_prefix(initial: &[u8], entries: &[Vec<u8>], prefix: usize, fresh: bool) -> Vec<u8> {
    let mut result = initial.to_vec();
    result[BITMAP_START..ENTRY_START].fill(0);
    if fresh {
        result[ENTRY_START..].fill(0);
    }
    let mut length = 0;
    for (position, entry) in entries.iter().enumerate() {
        let shared = if position == 0 { 0 } else { prefix };
        assert_eq!(entry[..shared], entries[0][..shared]);
        let suffix = &entry[shared..];
        result[ENTRY_START + length..ENTRY_START + length + suffix.len()].copy_from_slice(suffix);
        length += suffix.len();
        result[BITMAP_START + length / u8::BITS as usize] |= 1 << (length % u8::BITS as usize);
    }
    result[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
        .copy_from_slice(&((PAGE - ENTRY_START - length) as u16).to_le_bytes());
    result[PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()]
        .copy_from_slice(&(prefix as u16).to_le_bytes());
    result
}

#[test]
fn valid_unnecessary_fresh_prefix_does_not_replace_independent_seed_authority() {
    let model = source(6, Kind::Int64, vec![400], false);
    let before = model.bytes.clone();
    let requests = [request(&model)];
    let operation = FixedOperation {
        table: "Items",
        column: "Score",
        requests: &requests,
    };
    let image = crate::test_support::split_model::capture(&before);
    let structural = preflight(&image, &operation, &mut budget(), &|| false).unwrap();
    let mut staged =
        stage_checked::<8>(&image, &operation, structural, &mut budget(), &|| false).unwrap();
    let valid = finish::bound(
        crate::test_support::split_model::capture(&before),
        &operation,
        &staged,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    checked_values(valid.candidate_bytes().to_vec(), 400, i64::MAX);
    let mut values = model.values.clone();
    values[0] = Some(i64::MAX);
    let entries = model::ordered(&model, &values, false);
    let right = staged
        .plan
        .added
        .iter_mut()
        .find(|page| page.number as usize == model.eof)
        .unwrap();
    assert_eq!(
        &right.bytes[PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()],
        &[0, 0]
    );
    assert_eq!(entries[200..].iter().map(Vec::len).sum::<usize>(), 2600);
    right.bytes = repack_with_prefix(&right.bytes, &entries[200..], 1, true);
    checked_values(compose(&before, &staged), 400, i64::MAX);
    assert_eq!(
        finish::bound(image, &operation, &staged, &mut budget(), &|| false).err(),
        Some(EditError::Preservation)
    );
}

#[test]
fn valid_alternative_retained_prefix_does_not_replace_original_seed_authority() {
    let mut model = source(6, Kind::Int64, vec![400], false);
    model.replacement = 1;
    let before = model.bytes.clone();
    let requests = [request(&model)];
    let operation = FixedOperation {
        table: "Items",
        column: "Score",
        requests: &requests,
    };
    let image = crate::test_support::split_model::capture(&before);
    let structural = preflight(&image, &operation, &mut budget(), &|| false).unwrap();
    let mut staged =
        stage_checked::<8>(&image, &operation, structural, &mut budget(), &|| false).unwrap();
    let valid = finish::bound(
        crate::test_support::split_model::capture(&before),
        &operation,
        &staged,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    checked_values(valid.candidate_bytes().to_vec(), 400, 1);
    assert!(staged.plan.added.is_empty());
    let mut values = model.values.clone();
    values[0] = Some(1);
    let entries = model::ordered(&model, &values, false);
    let start = ROOTS[0] * PAGE;
    let current = compose(&before, &staged);
    assert_eq!(
        &current[start + PREFIX_OFFSET..start + PREFIX_OFFSET + size_of::<u16>()],
        &[8, 0]
    );
    let original = &before[start..start + PAGE];
    let after = repack_with_prefix(original, &entries, 7, false);
    staged
        .plan
        .patches
        .retain(|patch| patch.offset / PAGE != ROOTS[0]);
    staged
        .plan
        .patches
        .push(crate::table::indexes::IndexDataPatch {
            offset: start,
            before: original.to_vec(),
            after,
        });
    checked_values(compose(&before, &staged), 400, 1);
    assert_eq!(
        finish::bound(image, &operation, &staged, &mut budget(), &|| false).err(),
        Some(EditError::Preservation)
    );
}
