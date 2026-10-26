//! Authored linked-level packets and whole-image goldens, separate from engine code.
use super::{contraction_model, support, tree_model};
use std::{collections::BTreeMap, io::Cursor};
use tablecraft_ace::{Ace16DeletePreview, EditError, check_table, read_table_index};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

pub use contraction_model::{budget, capture, preview};

pub const PAGE_BYTES: usize = 4096;
pub const ROOT: u32 = 13;
pub const INTERNALS: [u32; 3] = [17, 18, 23];
pub const LEAVES: [u32; 4] = [15, 16, 19, 22];
pub const PREVIOUS: usize = 12;
pub const NEXT: usize = 16;
pub const TAIL: usize = 20;
pub const HEIGHT: usize = 26;
const FILE_PAGES: usize = 24;
const ENTRY_START: usize = 480;
const BITMAP_START: usize = 27;
const FREE_OFFSET: usize = 2;
const PREFIX_OFFSET: usize = 24;
const DATA_PAGE: usize = 12;
const TABLE_PAGE: usize = 10;
const TABLE_ROWS_OFFSET: usize = 16;
const SLOT_START: usize = 14;
const TOMBSTONE: u16 = 0xc000;

pub struct Model {
    pub base: contraction_model::Model,
    pub linked: bool,
}

fn links(order: &[u32], number: u32) -> (u32, u32) {
    let position = order.iter().position(|page| *page == number).unwrap();
    let previous = position.checked_sub(1).map_or(0, |index| order[index]);
    let next = order.get(position + 1).copied().unwrap_or(0);
    (previous, next)
}

fn dfs_levels(tree: &tree_model::Model, retired: &[u32]) -> BTreeMap<u8, Vec<u32>> {
    fn visit(
        tree: &tree_model::Model,
        retired: &[u32],
        number: u32,
        levels: &mut BTreeMap<u8, Vec<u32>>,
    ) -> u8 {
        let height = if let Some((_, children, _)) = tree.nodes.iter().find(|node| node.0 == number)
        {
            children
                .iter()
                .filter(|child| !retired.contains(child))
                .map(|child| visit(tree, retired, *child, levels) + 1)
                .max()
                .unwrap_or(0)
        } else {
            assert!(tree.leaves.iter().any(|leaf| leaf.0 == number));
            0
        };
        levels.entry(height).or_default().push(number);
        height
    }

    let mut levels = BTreeMap::new();
    visit(tree, retired, ROOT, &mut levels);
    levels
}

pub fn database(code: u8, descending: bool, linked: bool, indirect: bool) -> Model {
    let mut base =
        contraction_model::database(code, descending, true, true, true, indirect, indirect);
    let order = base.tree.ordered.clone();
    base.tree.leaves = LEAVES
        .iter()
        .enumerate()
        .map(|(position, &page)| (page, order[position * 2..position * 2 + 2].to_vec()))
        .collect();
    base.tree.nodes = vec![
        (ROOT, INTERNALS.to_vec(), code == 6),
        (17, vec![15], false),
        (18, vec![16, 19], false),
        (23, vec![22], true),
    ];
    base.tree.bytes.resize(FILE_PAGES * PAGE_BYTES, 0);
    base.tree.bytes[base.owned_bitmap..base.owned_bitmap + FILE_PAGES.div_ceil(8)].fill(0);
    for page in 0..FILE_PAGES {
        base.tree.bytes[base.global_bitmap + page / 8] &= !(1 << (page % 8));
    }
    for page in LEAVES.into_iter().chain([ROOT]).chain(INTERNALS) {
        base.tree.bytes[base.owned_bitmap + page as usize / 8] |= 1 << (page % 8);
    }
    let levels = dfs_levels(&base.tree, &[]);
    for (page, rows) in &base.tree.leaves {
        let entries = rows
            .iter()
            .map(|&row| tree_model::entry(base.tree.keys[row], row, descending))
            .collect::<Vec<_>>();
        let (previous, next) = links(&levels[&0], *page);
        let offset = *page as usize * PAGE_BYTES;
        base.tree.bytes[offset..offset + PAGE_BYTES]
            .copy_from_slice(&tree_model::page(0, &entries, previous, next, 0));
    }
    for (page, children, bounded) in base.tree.nodes.iter().rev() {
        let mut entries = Vec::new();
        let bound_count = children.len() - usize::from(!*bounded);
        for child in children.iter().take(bound_count) {
            let row = tree_model::subtree_rows(*child, &base.tree.leaves, &base.tree.nodes)
                .last()
                .copied()
                .unwrap();
            let mut entry = tree_model::entry(base.tree.keys[row], row, descending);
            entry.extend(child.to_be_bytes());
            entries.push(entry);
        }
        let tail = if *bounded {
            0
        } else {
            *children.last().unwrap()
        };
        let height = if *page == ROOT { 2 } else { 1 };
        let (previous, next) = if linked && *page != ROOT {
            links(&levels[&height], *page)
        } else {
            (0, 0)
        };
        let offset = *page as usize * PAGE_BYTES;
        base.tree.bytes[offset..offset + PAGE_BYTES]
            .copy_from_slice(&tree_model::page(height, &entries, previous, next, tail));
    }
    Model { base, linked }
}

pub fn requests(model: &Model, deleted: &[usize]) -> Vec<tablecraft_ace::RowDelete> {
    contraction_model::requests(&model.base, deleted)
}

pub fn candidate(model: &Model, deleted: &[usize]) -> Result<Ace16DeletePreview, EditError> {
    contraction_model::candidate(&model.base, deleted)
}

pub fn branch(model: &Model, number: u32) -> Vec<usize> {
    tree_model::subtree_rows(number, &model.base.tree.leaves, &model.base.tree.nodes)
}

fn repack(original: &[u8], entries: &[Vec<u8>]) -> Vec<u8> {
    let mut page = original.to_vec();
    page[BITMAP_START..ENTRY_START].fill(0);
    let mut length = 0;
    for entry in entries {
        page[ENTRY_START + length..ENTRY_START + length + entry.len()].copy_from_slice(entry);
        length += entry.len();
        page[BITMAP_START + length / 8] |= 1 << (length % 8);
    }
    support::put16(
        &mut page,
        FREE_OFFSET,
        (PAGE_BYTES - ENTRY_START - length) as u16,
    );
    support::put16(&mut page, PREFIX_OFFSET, 0);
    page
}

pub fn verify(model: &Model, candidate: &Ace16DeletePreview, deleted: &[usize], retired: &[u32]) {
    let tree = &model.base.tree;
    let original = &tree.bytes;
    let actual = candidate.candidate_bytes();
    let mut expected = original.clone();
    support::put32(
        &mut expected,
        TABLE_PAGE * PAGE_BYTES + TABLE_ROWS_OFFSET,
        8 - deleted.len() as u32,
    );
    for &row in deleted {
        let offset = DATA_PAGE * PAGE_BYTES + SLOT_START + row * size_of::<u16>();
        let word = u16::from_le_bytes(original[offset..offset + 2].try_into().unwrap());
        support::put16(&mut expected, offset, word | TOMBSTONE);
    }
    for &page in retired {
        assert_ne!(page, ROOT);
        expected[model.base.owned_bitmap + page as usize / 8] &= !(1 << (page % 8));
        expected[model.base.global_bitmap + page as usize / 8] |= 1 << (page % 8);
        expected[page as usize * PAGE_BYTES..page as usize * PAGE_BYTES + 4].fill(0);
    }
    let levels = dfs_levels(tree, retired);
    for (number, rows) in &tree.leaves {
        if retired.contains(number) {
            continue;
        }
        let entries = rows
            .iter()
            .copied()
            .filter(|row| !deleted.contains(row))
            .map(|row| tree_model::entry(tree.keys[row], row, tree.descending))
            .collect::<Vec<_>>();
        assert!(!entries.is_empty());
        let offset = *number as usize * PAGE_BYTES;
        let mut page = repack(&original[offset..offset + PAGE_BYTES], &entries);
        let (previous, next) = links(&levels[&0], *number);
        support::put32(&mut page, PREVIOUS, previous);
        support::put32(&mut page, NEXT, next);
        expected[offset..offset + PAGE_BYTES].copy_from_slice(&page);
    }
    for (number, children, bounded) in &tree.nodes {
        if retired.contains(number) {
            continue;
        }
        let offset = *number as usize * PAGE_BYTES;
        let children = children
            .iter()
            .copied()
            .filter(|page| !retired.contains(page))
            .collect::<Vec<_>>();
        let mut entries = Vec::new();
        if !children.is_empty() {
            let bound_count = children.len() - usize::from(!*bounded);
            for child in children.iter().take(bound_count) {
                let row = branch(model, *child)
                    .into_iter()
                    .rfind(|row| !deleted.contains(row))
                    .unwrap();
                let mut entry = tree_model::entry(tree.keys[row], row, tree.descending);
                entry.extend(child.to_be_bytes());
                entries.push(entry);
            }
        }
        let mut page = repack(&original[offset..offset + PAGE_BYTES], &entries);
        if children.is_empty() {
            assert_eq!(*number, ROOT);
            page[0] = 4;
            page[HEIGHT] = 0;
            support::put32(&mut page, PREVIOUS, 0);
            support::put32(&mut page, NEXT, 0);
            support::put32(&mut page, TAIL, 0);
        } else {
            let tail = if *bounded {
                0
            } else {
                *children.last().unwrap()
            };
            support::put32(&mut page, TAIL, tail);
            let (previous, next) = if model.linked && *number != ROOT {
                links(&levels[&1], *number)
            } else {
                (0, 0)
            };
            support::put32(&mut page, PREVIOUS, previous);
            support::put32(&mut page, NEXT, next);
        }
        expected[offset..offset + PAGE_BYTES].copy_from_slice(&page);
    }
    assert_eq!(candidate.before_bytes(), original);
    assert_eq!(candidate.deleted_rows(), deleted.len());
    assert_eq!(actual, expected, "complete independent byte golden");
    let ids = tree
        .ordered
        .iter()
        .filter(|row| !deleted.contains(row))
        .map(|row| Value::Int32(*row as i32 + 1))
        .collect::<Vec<_>>();
    for name in ["ScoreKey", "ScoreAlias"] {
        let rows = read_table_index(
            &mut Cursor::new(actual),
            CatalogScope::new(1),
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let actual_ids = rows
            .rows()
            .iter()
            .map(|row| row[0].clone())
            .collect::<Vec<_>>();
        assert_eq!(actual_ids, ids);
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
