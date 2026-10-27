//! Real owned key profiles test wire transport, never new public write admission.
use super::super::{Plan, borrowed_reference, edit_prepared, leaf_entries, nonroot, pages};
use super::{maps, nonroot as audit_nonroot};
use crate::{
    EditError, IndexCheckStatus, InspectError, check_table,
    format::reader::Reader,
    table::indexes::{
        binary::{self, BinaryIndexSelection, BinaryProfile},
        mutation::{
            retained::{Preserved, RetainedProfile},
            tree::Tree,
        },
        page::Entry,
        text::{self, TextProfile},
    },
    test_support::encoded::model,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn fresh(height: u8, entries: &[Vec<u8>], tail: u32) -> Vec<u8> {
    let mut expected = model::page(height, entries, 0, 0, tail);
    let end = 480 + entries.iter().map(Vec::len).sum::<usize>();
    expected[end..].fill(0);
    expected
}

fn transport<P: RetainedProfile, const N: usize>(model: model::Model, profile: P) {
    let checked = check_table(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Items",
        &mut model::budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let image = model::capture(&model.bytes);
    let mut source = image.reader();
    let mut budget = model::budget();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: &mut budget,
        cancelled: &|| false,
        reads: 0,
    };
    let index = checked.indexes()[0].index();
    let mut keys = Vec::new();
    let tree = Tree::read::<P, N>(&image, &mut reader, 10, 13, &profile, 0, &mut keys).unwrap();
    let mut plan = Plan::new(&image, 10);
    let mut patches = Vec::new();
    nonroot::edit(
        &nonroot::Operation::<P, N> {
            image: &image,
            index,
            profile: &profile,
            keys: &keys,
        },
        &tree,
        &mut plan,
        &mut reader,
        &mut patches,
    )
    .unwrap();
    assert!(patches.is_empty());
    assert!(plan.added.is_empty());
    assert!(plan.map_edits.is_empty());
    edit_prepared(
        &nonroot::Operation::<P, N> {
            image: &image,
            index,
            profile: &profile,
            keys: &keys,
        },
        &tree,
        1,
        &mut patches,
        &mut plan,
        &mut reader,
    )
    .unwrap();
    assert!(patches.is_empty());
    assert!(plan.added.is_empty());
    let mut next_eof = image.catalog().header().page_count();
    let mut map_edits = maps::Expected::default();
    let added = audit_nonroot::prove(
        &audit_nonroot::Operation::<P, N> {
            image: &image,
            tree: &tree,
            keys: &keys,
            profile: &profile,
            owner: 10,
            usage: index.usage_map(),
        },
        &mut reader,
        &mut audit_nonroot::Progress {
            next_eof: &mut next_eof,
            map_edits: &mut map_edits,
        },
    )
    .unwrap();
    assert!(added.is_empty());
    assert_eq!(next_eof, 20);

    let entries = leaf_entries(&keys, reader.budget, reader.cancelled).unwrap();
    let leaf = pages::leaf::<P, N>(
        10,
        0,
        0,
        &entries,
        &profile,
        reader.budget,
        reader.cancelled,
    )
    .unwrap();
    let wire: Vec<_> = model
        .order
        .iter()
        .map(|&row| model::wire(&model, row))
        .collect();
    assert_eq!(leaf, fresh(0, &wire, 0));

    for bounded in [false, true] {
        let mut entries = Vec::new();
        let mut expected = Vec::new();
        for (position, child) in [15_u32, 16, 19].into_iter().enumerate() {
            if !bounded && position == 2 {
                break;
            }
            let maximum = position * 3 + 2;
            entries.push(Entry {
                value: borrowed_reference(&keys[maximum]),
                child,
            });
            let mut packet = wire[maximum].clone();
            packet.extend(child.to_be_bytes());
            expected.push(packet);
        }
        let tail = if bounded { 0 } else { 19 };
        let node = pages::node::<P, N>(
            &pages::NodeHeader {
                owner: 10,
                height: 1,
                links: (0, 0),
                tail,
            },
            &entries,
            &profile,
            reader.budget,
            reader.cancelled,
        )
        .unwrap();
        assert_eq!(node, fresh(1, &expected, tail));
    }
}

#[test]
fn owned_general_keys_preserve_full_wire_row_maxima_and_original_tree_forms() {
    for family in [0, 1] {
        for descending in [false, true] {
            for node_form in [None, Some(false), Some(true)] {
                let mut original = model::text(family, descending, true);
                if let Some(bounded) = node_form {
                    original = model::tree(original, bounded);
                }
                let checked = check_table(
                    &mut Cursor::new(&original.bytes),
                    CatalogScope::new(1),
                    "Items",
                    &mut model::budget(),
                    &|| false,
                )
                .unwrap();
                let profile = Preserved(
                    TextProfile::new(1, &checked.schema().columns()[1], descending).unwrap(),
                );
                transport::<_, { text::MAX_ENTRY_BYTES }>(original, profile);
            }
        }
    }
}

#[test]
fn owned_binary_keys_keep_empty_order_and_complete_segment_wires() {
    for descending in [false, true] {
        for node_form in [None, Some(false), Some(true)] {
            let mut original = model::binary(descending, true);
            if let Some(bounded) = node_form {
                original = model::tree(original, bounded);
            }
            let profile = Preserved(BinaryProfile {
                column: 1,
                descending,
                maximum: 255,
                selection: BinaryIndexSelection::All,
            });
            transport::<_, { binary::MAX_ENTRY_BYTES }>(original, profile);
        }
    }
}

#[test]
fn borrowed_owned_key_output_still_pays_allocation_and_checks_cancellation() {
    let original = model::binary(false, false);
    let image = model::capture(&original.bytes);
    let mut source = image.reader();
    let profile = Preserved(BinaryProfile {
        column: 1,
        descending: false,
        maximum: 255,
        selection: BinaryIndexSelection::All,
    });
    let mut budget = model::budget();
    let mut keys = Vec::new();
    Tree::read::<_, { binary::MAX_ENTRY_BYTES }>(
        &image,
        &mut Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget: &mut budget,
            cancelled: &|| false,
            reads: 0,
        },
        10,
        13,
        &profile,
        0,
        &mut keys,
    )
    .unwrap();
    let entries = leaf_entries(&keys, &mut budget, &|| false).unwrap();
    for mut limited in [
        ResourceBudget::new(0, 100, 100_000),
        ResourceBudget::new(1_000_000, 100, 0),
    ] {
        assert!(matches!(
            pages::leaf::<_, { binary::MAX_ENTRY_BYTES }>(
                10,
                0,
                0,
                &entries,
                &profile,
                &mut limited,
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    assert!(matches!(
        pages::leaf::<_, { binary::MAX_ENTRY_BYTES }>(
            10,
            0,
            0,
            &entries,
            &profile,
            &mut model::budget(),
            &|| true
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    pages::leaf::<_, { binary::MAX_ENTRY_BYTES }>(
        10,
        0,
        0,
        &entries,
        &profile,
        &mut model::budget(),
        &|| false,
    )
    .unwrap();
}
