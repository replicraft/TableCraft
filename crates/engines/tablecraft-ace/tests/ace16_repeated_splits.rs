#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "ace16_fixed_split_contract/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "ace16_split_int32_contract/model.rs"]
mod root_model;
mod support;

use model::{Kind, Model, PAGE, ROOTS, budget, check, ordered, request, source};
use std::io::Cursor;
use tablecraft_ace::{FixedScalarUpdate, IndexCheckStatus, check_table, read_table};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

const KIND_OFFSET: usize = 0;
const FREE_OFFSET: usize = 2;
const TAIL_OFFSET: usize = 20;
const HEIGHT_OFFSET: usize = 26;
const CHILD_BYTES: usize = size_of::<u32>();
const DEPTH_LIMIT: usize = 16;

fn descendants(bytes: &[u8], number: usize) -> Vec<usize> {
    let entries = root_model::entries(bytes, number);
    let mut children = entries
        .iter()
        .map(|entry| {
            u32::from_be_bytes(entry[entry.len() - CHILD_BYTES..].try_into().unwrap()) as usize
        })
        .collect::<Vec<_>>();
    let tail = root_model::get32(bytes, number * PAGE + TAIL_OFFSET) as usize;
    if tail != 0 {
        children.push(tail);
    }
    children
}

fn flattened(bytes: &[u8], number: usize, depth: usize) -> Vec<Vec<u8>> {
    assert!(depth < DEPTH_LIMIT);
    if bytes[number * PAGE + KIND_OFFSET] == 4 {
        return root_model::entries(bytes, number);
    }
    descendants(bytes, number)
        .into_iter()
        .flat_map(|child| flattened(bytes, child, depth + 1))
        .collect()
}

fn candidate(mut model: Model, added: usize) -> Model {
    check(&model);
    let before = model.bytes.clone();
    let preview = root_model::capture(&before)
        .preview_ace16_split_fixed_updates(
            "Items",
            "Score",
            &[request(&model)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(preview.changed_rows(), 1);
    assert_eq!(preview.candidate_bytes().len(), before.len() + added * PAGE);
    assert_eq!(model.bytes, before);
    for body in preview.candidate_bytes()[before.len()..]
        .as_chunks::<PAGE>()
        .0
    {
        let free = u16::from_le_bytes(
            body[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
                .try_into()
                .unwrap(),
        ) as usize;
        assert!(body[PAGE - free..].iter().all(|&byte| byte == 0));
    }
    model.values[0] = Some(model.replacement);
    model.bytes = preview.candidate_bytes().to_vec();
    check(&model);
    for (physical, &root) in ROOTS.iter().enumerate() {
        assert_eq!(
            flattened(&model.bytes, root, 0),
            ordered(&model, &model.values, physical == 1)
        );
    }
    model
}

#[test]
fn wide_root_halves_split_again_with_right_then_left_allocation_and_zero_padding() {
    for code in [5, 6] {
        for kind in [Kind::Int64, Kind::Currency] {
            let original = source(code, kind, vec![900], false);
            let eof = original.bytes.len() / PAGE;
            let actual = candidate(original, 6);
            for (physical, &root) in ROOTS.iter().enumerate() {
                assert_eq!(actual.bytes[root * PAGE + KIND_OFFSET], 3);
                assert_eq!(actual.bytes[root * PAGE + HEIGHT_OFFSET], 1);
                assert_eq!(root_model::entries(&actual.bytes, root).len(), 2);
                let base = eof + physical * 3;
                let children = descendants(&actual.bytes, root);
                let expected = if physical == 0 {
                    vec![base + 1, base + 2, base]
                } else {
                    vec![base + 2, base + 1, base]
                };
                assert_eq!(children, expected);
                let counts = children
                    .iter()
                    .map(|&page| root_model::entries(&actual.bytes, page).len())
                    .collect::<Vec<_>>();
                assert_eq!(
                    counts,
                    if physical == 0 {
                        vec![450, 225, 225]
                    } else {
                        vec![225, 225, 450]
                    }
                );
            }
        }
    }
}

#[test]
fn the_original_wide_parent_max_half_normalizes_without_changing_source_rows() {
    for code in [5, 6] {
        for kind in [Kind::Int64, Kind::Currency] {
            let original = source(code, kind, vec![1; 600], false);
            let actual = candidate(original, 5);
            for (physical, &root) in ROOTS.iter().enumerate() {
                assert_eq!(actual.bytes[root * PAGE + HEIGHT_OFFSET], 2);
                assert_eq!(
                    root_model::get32(&actual.bytes, root * PAGE + TAIL_OFFSET),
                    0
                );
                let children = descendants(&actual.bytes, root);
                assert_eq!(children.len(), if physical == 0 { 2 } else { 3 });
                let leaves = children
                    .iter()
                    .map(|&child| descendants(&actual.bytes, child).len())
                    .collect::<Vec<_>>();
                assert_eq!(
                    leaves,
                    if physical == 0 {
                        vec![300, 300]
                    } else {
                        vec![150, 150, 300]
                    }
                );
            }
        }
    }
}

fn compressed_source(kind: Kind) -> Model {
    // Independent authored source: combine two valid 800-entry leaves into one
    // root using common full key+ROW prefix, then remove their authored page bits.
    // Accepted public inputs and the shared fixture source are never changed.
    let mut model = source(6, kind, vec![800, 800], false);
    check(&model);
    for (physical, &root) in ROOTS.iter().enumerate() {
        let keys = ordered(&model, &model.values, physical == 1);
        let prefix = model::common(&keys, false);
        assert_eq!(prefix, kind.width() + 3);
        let body = model::repack(&model::blank(0, true), &keys, prefix);
        model::install(&mut model.bytes, root, &body);
        for number in 32..36 {
            let at = root_model::map_byte(&model.bytes, model::MAPS[physical], number);
            model.bytes[at] &= !(1 << (number % 8));
        }
    }
    // The shared builder rounds its EOF up to eight pages. Every discarded
    // page, including its unused alignment pages, becomes future-free here.
    for number in 32..model.eof {
        let at = root_model::map_byte(&model.bytes, 1, number);
        model.bytes[at] |= 1 << (number % 8);
    }
    model.bytes.truncate(32 * PAGE);
    model.eof = 32;
    model.root_leaf = true;
    model.counts = vec![1600];
    check(&model);
    model
}

#[test]
fn appended_left_pages_join_the_same_pass_before_a_retained_right_revisit() {
    for kind in [Kind::Int64, Kind::Currency] {
        let mut model = compressed_source(kind);
        let before = model.bytes.clone();
        let updates = [
            FixedScalarUpdate::new(0, kind.value(0), kind.value(i64::MIN)),
            FixedScalarUpdate::new(1, kind.value(0), kind.value(i64::MAX)),
        ];
        let preview = root_model::capture(&before)
            .preview_ace16_split_fixed_updates("Items", "Score", &updates, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(preview.changed_rows(), 2);
        assert_eq!(preview.candidate_bytes().len(), before.len() + 12 * PAGE);
        assert_eq!(model.bytes, before);
        model.bytes = preview.candidate_bytes().to_vec();
        model.values[0] = Some(i64::MIN);
        model.values[1] = Some(i64::MAX);
        check(&model);
        for (physical, &root) in ROOTS.iter().enumerate() {
            let base = 32 + physical * 6;
            let expected = vec![base + 4, base + 3, base + 1, base + 2, base + 5, base];
            let children = descendants(&model.bytes, root);
            assert_eq!(children, expected);
            assert_eq!(
                children
                    .iter()
                    .map(|&page| root_model::entries(&model.bytes, page).len())
                    .collect::<Vec<_>>(),
                vec![200, 200, 400, 400, 200, 200]
            );
            assert_eq!(
                flattened(&model.bytes, root, 0),
                ordered(&model, &model.values, physical == 1)
            );
        }
    }
}

#[test]
fn repeated_capacity_uses_each_closed_integral_width_and_preserves_old_int32_output() {
    for kind in [Kind::Byte, Kind::Int16, Kind::Int32] {
        let original = compressed_source(kind);
        let before = original.bytes.clone();
        let actual = candidate(original, 6);
        for (physical, &root) in ROOTS.iter().enumerate() {
            let children = descendants(&actual.bytes, root);
            let counts = children
                .iter()
                .map(|&page| root_model::entries(&actual.bytes, page).len())
                .collect::<Vec<_>>();
            assert_eq!(
                counts,
                if physical == 0 {
                    vec![800, 400, 400]
                } else {
                    vec![400, 400, 800]
                }
            );
        }
        if matches!(kind, Kind::Int32) {
            let old = root_model::capture(&before)
                .preview_ace16_split_int32_updates(
                    "Items",
                    "Score",
                    &[tablecraft_ace::Int32Update::new(0, 0, i32::MAX)],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(old.candidate_bytes(), actual.bytes);
        }
    }
}
