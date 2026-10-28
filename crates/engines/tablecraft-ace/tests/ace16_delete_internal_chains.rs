#![cfg(feature = "ace16-write-preview")]

#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "ace16_delete_contraction_contract/model.rs"]
mod model;
use tablecraft_testkit::access as support;
#[allow(dead_code)]
#[path = "ace16_delete_tree_contract/model.rs"]
mod tree_model;

use std::io::Cursor;
use tablecraft_ace::{EditError, InspectError, check_table, read_table_index};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

fn linked(code: u8, descending: bool, bounded: bool) -> model::Model {
    let mut model = model::database(code, descending, bounded, true, true, false, false);
    support::put32(&mut model.tree.bytes, 17 * 4096 + 16, 18);
    support::put32(&mut model.tree.bytes, 18 * 4096 + 12, 17);
    model
}

fn word(bytes: &[u8], page: usize, offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[page * 4096 + offset..page * 4096 + offset + 4]
            .try_into()
            .unwrap(),
    )
}

#[test]
fn linked_internal_neighbors_repair_after_first_last_and_whole_branch_retirement() {
    for code in [5, 6] {
        for descending in [false, true] {
            for bounded in [false, true] {
                let model = linked(code, descending, bounded);
                let left = model.tree.leaves[..2]
                    .iter()
                    .flat_map(|leaf| leaf.1.iter().copied())
                    .collect::<Vec<_>>();
                let right = model.tree.leaves[2].1.clone();
                for (deleted, surviving, retired) in [(left, 18, 17), (right, 17, 18)] {
                    let output = model::candidate(&model, &deleted).unwrap();
                    let after = output.candidate_bytes();
                    assert_eq!(word(after, surviving, 12), 0);
                    assert_eq!(word(after, surviving, 16), 0);
                    assert_eq!(&after[retired * 4096..retired * 4096 + 4], &[0; 4]);
                    assert_eq!(
                        &after[retired * 4096 + 4..(retired + 1) * 4096],
                        &model.tree.bytes[retired * 4096 + 4..(retired + 1) * 4096]
                    );
                    assert_eq!(after[13 * 4096 + 26], 2);
                    assert_eq!(after[surviving * 4096 + 26], 1);
                    let table = read_table_index(
                        &mut Cursor::new(after),
                        CatalogScope::new(1),
                        "Items",
                        "ScoreKey",
                        &mut model::budget(),
                        &|| false,
                    )
                    .unwrap();
                    let actual = table
                        .rows()
                        .iter()
                        .map(|row| row[0].clone())
                        .collect::<Vec<_>>();
                    let expected = model
                        .tree
                        .leaves
                        .iter()
                        .flat_map(|leaf| leaf.1.iter())
                        .filter(|row| !deleted.contains(row))
                        .map(|row| Value::Int32(*row as i32 + 1))
                        .collect::<Vec<_>>();
                    assert_eq!(actual, expected);
                }
                let deleted = (0..8).collect::<Vec<_>>();
                let output = model::candidate(&model, &deleted).unwrap();
                assert_eq!(&output.candidate_bytes()[13 * 4096..13 * 4096 + 2], &[4, 1]);
                assert_eq!(output.candidate_bytes()[13 * 4096 + 26], 0);
                assert_eq!(word(output.candidate_bytes(), 13, 12), 0);
                assert_eq!(word(output.candidate_bytes(), 13, 16), 0);
            }
        }
    }
}

#[test]
fn incomplete_internal_chains_refuse_contraction_without_changing_legacy_noops() {
    let model = linked(6, false, true);
    let deleted = model.tree.leaves[2].1.clone();
    for (page, offset, target) in [(17, 16, 19), (18, 12, 0), (17, 12, 18), (13, 16, 17)] {
        let mut malformed = linked(6, false, true);
        support::put32(&mut malformed.tree.bytes, page * 4096 + offset, target);
        check_table(
            &mut Cursor::new(&malformed.tree.bytes),
            CatalogScope::new(1),
            "Items",
            &mut model::budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            model::candidate(&malformed, &deleted).err(),
            Some(EditError::Inspect(InspectError::Malformed))
        );
        assert_eq!(
            model::capture(&malformed.tree.bytes)
                .preview_ace16_deletes("Items", &[], &mut model::budget(), &|| false)
                .unwrap()
                .candidate_bytes(),
            malformed.tree.bytes
        );
    }
}
