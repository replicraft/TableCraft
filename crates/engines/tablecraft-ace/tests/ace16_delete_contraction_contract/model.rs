//! Independent map/header/whole-image goldens over the authored eight-row tree.
use super::{support, tree_model};
use std::io::Cursor;
use tablecraft_ace::{Ace16DeletePreview, EditError, RowDelete, check_table, read_table_index};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

pub struct Model {
    pub tree: tree_model::Model,
    pub global_bitmap: usize,
    pub owned_bitmap: usize,
}

pub use tree_model::{budget, capture, preview};

pub fn database(
    code: u8,
    descending: bool,
    bounded: bool,
    deep: bool,
    alias: bool,
    indirect_global: bool,
    indirect_owned: bool,
) -> Model {
    let mut tree = tree_model::database(code, descending, bounded, deep, alias);
    let pages = 20 + usize::from(indirect_global) + usize::from(indirect_owned);
    tree.bytes.resize(pages * 4096, 0);
    // Actual reviewed map containers use owner0 for index maps and owner1
    // for the distinguished global map; they are not user-table records.
    support::put32(&mut tree.bytes, 14 * 4096 + 4, 0);
    support::put32(&mut tree.bytes, 11 * 4096 + 4, 0);
    let global_bitmap = if indirect_global {
        let pointer = 20_u32;
        let mut row = vec![1];
        row.extend(pointer.to_le_bytes());
        tree.bytes[4096..8192].copy_from_slice(&support::data_page(1, &[(0, row)]));
        tree.bytes[20 * 4096..20 * 4096 + 4].copy_from_slice(&[5, 1, 0, 0]);
        20 * 4096 + 4
    } else {
        tree.bytes[4096..8192].copy_from_slice(&support::data_page(1, &[(0, vec![0; 69])]));
        8192 - 64
    };
    let represented = if indirect_global { 4092 * 8 } else { 512 };
    for page in pages..represented {
        tree.bytes[global_bitmap + page / 8] |= 1 << (page % 8);
    }
    let owned_bitmap = if indirect_owned {
        let pointer = if indirect_global { 21_u32 } else { 20 };
        let mut row = vec![1];
        row.extend(pointer.to_le_bytes());
        tree.bytes[14 * 4096..15 * 4096].copy_from_slice(&support::data_page(0, &[(0, row)]));
        let start = pointer as usize * 4096;
        tree.bytes[start..start + 4].copy_from_slice(&[5, 1, 0, 0]);
        for page in tree
            .leaves
            .iter()
            .map(|leaf| leaf.0)
            .chain(tree.nodes.iter().map(|node| node.0))
        {
            tree.bytes[start + 4 + page as usize / 8] |= 1 << (page % 8);
        }
        start + 4
    } else {
        15 * 4096 - 3
    };
    Model {
        tree,
        global_bitmap,
        owned_bitmap,
    }
}

pub fn requests(model: &Model, deleted: &[usize]) -> Vec<RowDelete> {
    deleted
        .iter()
        .map(|&row| tree_model::request(&model.tree, row))
        .collect()
}

pub fn candidate(model: &Model, deleted: &[usize]) -> Result<Ace16DeletePreview, EditError> {
    preview(&model.tree.bytes, &requests(model, deleted))
}

fn repack(original: &[u8], entries: &[Vec<u8>]) -> Vec<u8> {
    let mut page = original.to_vec();
    page[27..480].fill(0);
    let mut length = 0;
    for entry in entries {
        page[480 + length..480 + length + entry.len()].copy_from_slice(entry);
        length += entry.len();
        page[27 + length / 8] |= 1 << (length % 8);
    }
    support::put16(&mut page, 2, (3616 - length) as u16);
    support::put16(&mut page, 24, 0);
    page
}

pub fn verify(model: &Model, candidate: &Ace16DeletePreview, deleted: &[usize], retired: &[u32]) {
    let original = &model.tree.bytes;
    let actual = candidate.candidate_bytes();
    assert_eq!(candidate.before_bytes(), original);
    assert_eq!(candidate.deleted_rows(), deleted.len());
    let mut expected = original.clone();
    support::put32(&mut expected, 10 * 4096 + 16, 8 - deleted.len() as u32);
    for &row in deleted {
        let offset = 12 * 4096 + 14 + row * 2;
        let word = u16::from_le_bytes(original[offset..offset + 2].try_into().unwrap());
        support::put16(&mut expected, offset, word | 0xc000);
    }
    for &page in retired {
        assert_ne!(page, 13, "the original root is never retired");
        assert!(
            model.tree.leaves.iter().any(|leaf| leaf.0 == page)
                || model.tree.nodes.iter().any(|node| node.0 == page)
        );
        expected[model.owned_bitmap + page as usize / 8] &= !(1 << (page % 8));
        expected[model.global_bitmap + page as usize / 8] |= 1 << (page % 8);
        expected[page as usize * 4096..page as usize * 4096 + 4].fill(0);
    }
    let live_leaves = model
        .tree
        .leaves
        .iter()
        .filter(|leaf| !retired.contains(&leaf.0))
        .collect::<Vec<_>>();
    for (position, leaf) in live_leaves.iter().enumerate() {
        let number = leaf.0;
        let rows = &leaf.1;
        let entries = rows
            .iter()
            .copied()
            .filter(|row| !deleted.contains(row))
            .map(|row| tree_model::entry(model.tree.keys[row], row, model.tree.descending))
            .collect::<Vec<_>>();
        assert!(!entries.is_empty());
        let offset = number as usize * 4096;
        let mut page = repack(&original[offset..offset + 4096], &entries);
        support::put32(
            &mut page,
            12,
            position.checked_sub(1).map_or(0, |i| live_leaves[i].0),
        );
        support::put32(
            &mut page,
            16,
            live_leaves.get(position + 1).map_or(0, |leaf| leaf.0),
        );
        expected[offset..offset + 4096].copy_from_slice(&page);
    }
    for (number, children, bounded) in &model.tree.nodes {
        if retired.contains(number) {
            continue;
        }
        let offset = *number as usize * 4096;
        let children = children
            .iter()
            .copied()
            .filter(|child| !retired.contains(child))
            .collect::<Vec<_>>();
        if children.is_empty() {
            assert_eq!(*number, 13);
            let mut page = repack(&original[offset..offset + 4096], &[]);
            page[..2].copy_from_slice(&[4, 1]);
            page[26] = 0;
            support::put32(&mut page, 12, 0);
            support::put32(&mut page, 16, 0);
            support::put32(&mut page, 20, 0);
            expected[offset..offset + 4096].copy_from_slice(&page);
            continue;
        }
        let bound_count = if *bounded {
            children.len()
        } else {
            children.len() - 1
        };
        let entries = children
            .iter()
            .take(bound_count)
            .map(|child| {
                let row = tree_model::subtree_rows(*child, &model.tree.leaves, &model.tree.nodes)
                    .into_iter()
                    .rfind(|row| !deleted.contains(row))
                    .unwrap();
                let mut entry = tree_model::entry(model.tree.keys[row], row, model.tree.descending);
                entry.extend(child.to_be_bytes());
                entry
            })
            .collect::<Vec<_>>();
        let mut page = repack(&original[offset..offset + 4096], &entries);
        support::put32(
            &mut page,
            20,
            if *bounded {
                0
            } else {
                *children.last().unwrap()
            },
        );
        expected[offset..offset + 4096].copy_from_slice(&page);
    }
    assert_eq!(
        actual, expected,
        "exact independently modelled candidate bytes"
    );
    assert_eq!(actual.len(), original.len());
    assert_eq!(
        &actual[11 * 4096..12 * 4096],
        &original[11 * 4096..12 * 4096]
    );
    assert_eq!(
        &actual[12 * 4096 + 30..13 * 4096],
        &original[12 * 4096 + 30..13 * 4096]
    );
    let expected_ids = model
        .tree
        .ordered
        .iter()
        .filter(|row| !deleted.contains(row))
        .map(|row| Value::Int32(*row as i32 + 1))
        .collect::<Vec<_>>();
    for name in if original[10 * 4096 + 47] == 2 {
        vec!["ScoreKey", "ScoreAlias"]
    } else {
        vec!["ScoreKey"]
    } {
        let rows = read_table_index(
            &mut Cursor::new(actual),
            CatalogScope::new(1),
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            rows.rows()
                .iter()
                .map(|row| row[0].clone())
                .collect::<Vec<_>>(),
            expected_ids
        );
    }
    check_table(
        &mut Cursor::new(actual),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
}
