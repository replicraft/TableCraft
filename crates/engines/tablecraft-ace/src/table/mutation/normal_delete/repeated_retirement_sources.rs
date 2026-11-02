//! Source-only eight-view recursive retirement gate; no mutation producer calls.
use crate::{
    IndexCheckStatus, NativeImage, check_table_constraints, inspect_table_index_counter,
    inspect_table_schema, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Cursor,
    path::PathBuf,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;
const ACTIVE: usize = 480;
const MASK: usize = 27;
const MASK_BYTES: usize = 453;
const SCOPE: CatalogScope = CatalogScope::new(263);

#[derive(Clone)]
struct Row {
    reference: u32,
    id: i32,
    offset: usize,
    raw: Vec<u8>,
}

struct Node {
    height: u8,
    prefix: usize,
    previous: u32,
    next: u32,
    tail: u32,
    children: Vec<u32>,
    keys: Vec<Vec<u8>>,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 10_000_000, 2_000_000_000)
}

fn word(raw: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(raw[at..at + 2].try_into().unwrap())
}

fn number(raw: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(raw[at..at + 4].try_into().unwrap())
}

fn key(id: i32, reference: u32) -> Vec<u8> {
    let mut result = vec![127];
    result.extend_from_slice(&((id as u32) ^ 0x8000_0000).to_be_bytes());
    result.extend_from_slice(&reference.to_be_bytes());
    result
}

fn record(id: i32) -> Vec<u8> {
    let mut raw = vec![2, 0];
    raw.extend_from_slice(&id.to_le_bytes());
    raw.extend_from_slice(&[6, 0, 6, 0, 1, 0, 3]);
    raw
}

fn indirect(raw: &[u8], start: usize, bitmap: u32) -> Vec<u32> {
    assert_eq!(raw[start], 1);
    assert_eq!(number(raw, start + 1), bitmap);
    assert_eq!(&raw[start + 5..start + 69], &[0; 64]);
    let storage = &raw[bitmap as usize * PAGE..(bitmap as usize + 1) * PAGE];
    assert_eq!(&storage[..4], &[5, 1, 0, 0]);
    (0..(PAGE - 4) * 8)
        .filter(|bit| storage[4 + bit / 8] & (1 << (bit % 8)) != 0)
        .map(|bit| bit as u32)
        .collect()
}

fn rows(raw: &[u8], definition: u32, owned: &[u32], live: usize) -> Vec<Row> {
    let mut result = Vec::new();
    let mut slots = 0;
    let mut deleted = 0;
    for &number in owned {
        let page = &raw[number as usize * PAGE..(number as usize + 1) * PAGE];
        assert_eq!(&page[..2], &[1, 1]);
        assert_eq!(self::number(page, 4), definition);
        let count = usize::from(word(page, 12));
        assert!(count <= 255);
        let directory_end = 14 + 2 * count;
        let mut end = PAGE;
        for slot in 0..count {
            let flags = word(page, 14 + 2 * slot);
            assert!(matches!(flags & 0xe000, 0 | 0x8000 | 0xc000));
            let start = usize::from(flags & 0x1fff);
            assert!(directory_end <= start && start <= end && end <= PAGE);
            slots += 1;
            if flags & 0x8000 != 0 {
                deleted += 1;
            } else {
                assert_eq!(flags & 0xe000, 0);
                let body = &page[start..end];
                assert_eq!(body.len(), 13);
                let id = i32::from_le_bytes(body[2..6].try_into().unwrap());
                assert_eq!(body, record(id));
                result.push(Row {
                    reference: number << 8 | slot as u32,
                    id,
                    offset: number as usize * PAGE + start,
                    raw: body.to_vec(),
                });
            }
            end = start;
        }
        assert_eq!(usize::from(word(page, 2)), end - directory_end);
    }
    assert_eq!(
        (slots, deleted, result.len()),
        (149184, 149184 - live, live)
    );
    assert!(result.windows(2).all(|pair| pair[0].id < pair[1].id));
    result
}

fn tree(raw: &[u8], owner: u32, page: u32, nodes: &mut BTreeMap<u32, Node>) -> Vec<Vec<u8>> {
    assert!(!nodes.contains_key(&page));
    let body = &raw[page as usize * PAGE..(page as usize + 1) * PAGE];
    assert!(matches!(body[0], 3 | 4));
    assert_eq!(body[1], 1);
    assert_eq!(number(body, 4), owner);
    assert_eq!(&body[8..12], &[0; 4]);
    let height = body[26];
    assert_eq!(body[0] == 4, height == 0);
    let prefix = usize::from(word(body, 24));
    assert!(prefix <= 9);
    let mut entries: Vec<Vec<u8>> = Vec::new();
    let mut end = 0;
    for bit in 0..MASK_BYTES * 8 {
        if body[MASK + bit / 8] & (1 << (bit % 8)) == 0 {
            continue;
        }
        assert!(end < bit && ACTIVE + bit <= PAGE);
        let mut entry = if entries.is_empty() {
            Vec::new()
        } else {
            entries[0][..prefix].to_vec()
        };
        entry.extend_from_slice(&body[ACTIVE + end..ACTIVE + bit]);
        assert_eq!(entry.len(), if height == 0 { 9 } else { 13 });
        assert_eq!(entry[0], 127);
        entries.push(entry);
        end = bit;
    }
    assert!(!entries.is_empty() || height == 0);
    assert_eq!(usize::from(word(body, 2)), PAGE - ACTIVE - end);
    let tail = number(body, 20);
    let children = if height == 0 {
        Vec::new()
    } else {
        entries
            .iter()
            .map(|entry| u32::from_be_bytes(entry[9..13].try_into().unwrap()))
            .collect()
    };
    let keys = entries
        .iter()
        .map(|entry| entry[..9].to_vec())
        .collect::<Vec<_>>();
    nodes.insert(
        page,
        Node {
            height,
            prefix,
            previous: number(body, 12),
            next: number(body, 16),
            tail,
            children: children.clone(),
            keys: keys.clone(),
        },
    );
    if height == 0 {
        assert_eq!(tail, 0);
        return keys;
    }
    let mut population = Vec::new();
    for (bound, child) in keys.iter().zip(children) {
        let nested = tree(raw, owner, child, nodes);
        assert_eq!(nodes[&child].height + 1, height);
        assert_eq!(nested.last().unwrap(), bound);
        population.extend(nested);
    }
    if tail != 0 {
        population.extend(tree(raw, owner, tail, nodes));
        assert_eq!(nodes[&tail].height + 1, height);
    }
    population
}

fn hex(raw: &[u8]) -> String {
    raw.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct Profile {
    header: u8,
    definition: u32,
    root: u32,
    parent: u32,
    tail: u32,
    next: u32,
    table_map: usize,
    index_map: usize,
    table_bitmap: u32,
    index_bitmap: u32,
    length: usize,
    hashes: [&'static str; 4],
}

fn profiles() -> [Profile; 2] {
    [
        Profile {
            header: 5,
            definition: 89,
            root: 91,
            parent: 820,
            tail: 100,
            next: 463,
            table_map: 372667,
            index_map: 372529,
            table_bitmap: 97,
            index_bitmap: 605,
            length: 4837376,
            hashes: [
                "ae73258f861d11d33b0dd3f2a48a5e5448d34534febe1aaccaf2c9abb1804372",
                "e70c247114df18faebb2889b821254818f6e46e77855aedf02aa0209b4d2eab2",
                "f1ef85ef43f55fc1d401861224f26af3066347d4835a0fe9dc7eea42cee0cc8b",
                "20a07f69ea9a7eb62509107768a2758a9d30c1c2e124833a6bb7b12e53684903",
            ],
        },
        Profile {
            header: 6,
            definition: 106,
            root: 108,
            parent: 838,
            tail: 118,
            next: 481,
            table_map: 442299,
            index_map: 442161,
            table_bitmap: 115,
            index_bitmap: 621,
            length: 4911104,
            hashes: [
                "263024334384b0d919f6a6765d2a733e9094aa4986f682fcba73943e8da2a078",
                "80b3298b6390fbe4646a28a22ae395def61f72ad699e2e3c7ac59904e54ba065",
                "3fd7b5cecd6ae9ff330b21e394126fdda962df7383c39d36fce35ada92386e17",
                "1a18df43bc9ede1ad8c18fcdbfad9546474aab398bbea987d45b21596b79c82a",
            ],
        },
    ]
}

fn set_number(raw: &mut [u8], at: usize, value: u32) {
    raw[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn frame(raw: &mut [u8], page: u32, seed: usize, entries: &[Vec<u8>]) {
    let base = page as usize * PAGE;
    let mut prefix = if entries.is_empty() { 0 } else { seed };
    for entry in entries {
        prefix = prefix.min(entry.len());
        prefix = (0..prefix)
            .take_while(|&at| entries[0][at] == entry[at])
            .count();
    }
    raw[base + MASK..base + ACTIVE].fill(0);
    let mut used = 0;
    for (ordinal, entry) in entries.iter().enumerate() {
        let wire = if ordinal == 0 {
            entry.as_slice()
        } else {
            &entry[prefix..]
        };
        raw[base + ACTIVE + used..base + ACTIVE + used + wire.len()].copy_from_slice(wire);
        used += wire.len();
        assert!(used <= PAGE - ACTIVE);
        raw[base + MASK + used / 8] |= 1 << (used % 8);
    }
    raw[base + 2..base + 4]
        .copy_from_slice(&u16::try_from(PAGE - ACTIVE - used).unwrap().to_le_bytes());
    raw[base + 24..base + 26].copy_from_slice(&u16::try_from(prefix).unwrap().to_le_bytes());
}

struct Replay<'a> {
    root: u32,
    original: &'a BTreeMap<u32, Node>,
    removed: &'a BTreeSet<u32>,
    retired: BTreeSet<u32>,
    output: Vec<u8>,
}

impl Replay<'_> {
    fn visit(&mut self, page: u32) -> Option<Vec<u8>> {
        let node = &self.original[&page];
        if node.height == 0 {
            let entries = node
                .keys
                .iter()
                .filter(|key| {
                    !self
                        .removed
                        .contains(&u32::from_be_bytes(key[5..9].try_into().unwrap()))
                })
                .cloned()
                .collect::<Vec<_>>();
            if entries.is_empty() {
                assert!(self.retired.insert(page));
                return None;
            }
            frame(&mut self.output, page, node.prefix, &entries);
            return entries.last().cloned();
        }

        let mut children = node.children.clone();
        if node.tail != 0 {
            children.push(node.tail);
        }
        let seed = node.prefix;
        let old_tail = node.tail;
        let mut kept = Vec::new();
        for child in children {
            if let Some(maximum) = self.visit(child) {
                kept.push((child, maximum));
            }
        }
        if kept.is_empty() {
            if page == self.root {
                frame(&mut self.output, page, 0, &[]);
                let base = page as usize * PAGE;
                self.output[base] = 4;
                self.output[base + 26] = 0;
                set_number(&mut self.output, base + 20, 0);
            } else {
                assert!(self.retired.insert(page));
            }
            return None;
        }

        let tail = if old_tail != 0 && kept.len() > 1 {
            kept.last().unwrap().0
        } else {
            0
        };
        let entries = kept
            .iter()
            .filter(|(child, _)| *child != tail)
            .map(|(child, bound)| {
                let mut entry = bound.clone();
                entry.extend_from_slice(&child.to_be_bytes());
                entry
            })
            .collect::<Vec<_>>();
        frame(&mut self.output, page, seed, &entries);
        set_number(&mut self.output, page as usize * PAGE + 20, tail);
        Some(kept.last().unwrap().1.clone())
    }
}

fn reconstruct(
    before: &[u8],
    profile: &Profile,
    selected: &[Row],
    nodes: &BTreeMap<u32, Node>,
) -> (Vec<u8>, BTreeSet<u32>) {
    let removed = selected
        .iter()
        .map(|row| row.reference)
        .collect::<BTreeSet<_>>();
    assert_eq!(removed.len(), selected.len());
    let mut replay = Replay {
        root: profile.root,
        original: nodes,
        removed: &removed,
        retired: BTreeSet::new(),
        output: before.to_vec(),
    };
    let maximum = replay.visit(profile.root);
    assert_eq!(maximum.is_some(), selected.len() != 798);
    assert!(!replay.retired.contains(&profile.root));

    for height in 0..=2 {
        let first = *nodes
            .iter()
            .find(|(_, node)| node.height == height && node.previous == 0)
            .unwrap()
            .0;
        let mut current = first;
        let mut kept = Vec::new();
        while current != 0 {
            if !replay.retired.contains(&current) {
                kept.push(current);
            }
            current = nodes[&current].next;
        }
        for (position, &page) in kept.iter().enumerate() {
            let previous = if position == 0 { 0 } else { kept[position - 1] };
            let next = kept.get(position + 1).copied().unwrap_or(0);
            set_number(&mut replay.output, page as usize * PAGE + 12, previous);
            set_number(&mut replay.output, page as usize * PAGE + 16, next);
        }
    }

    for row in selected {
        let page = row.reference >> 8;
        let slot = row.reference & 255;
        let at = page as usize * PAGE + 14 + 2 * slot as usize;
        let original = word(before, at);
        assert_eq!(original & 0xe000, 0);
        replay.output[at..at + 2].copy_from_slice(&(original | 0xc000).to_le_bytes());
    }
    set_number(
        &mut replay.output,
        profile.definition as usize * PAGE + 16,
        u32::try_from(798 - selected.len()).unwrap(),
    );
    let mut masks = BTreeMap::<usize, u8>::new();
    for &page in &replay.retired {
        for (bitmap, owned) in [(profile.index_bitmap, true), (513, false)] {
            let at = bitmap as usize * PAGE + 4 + page as usize / 8;
            let mask = 1 << (page % 8);
            assert_eq!(before[at] & mask != 0, owned);
            let previous = masks.entry(at).or_default();
            assert_eq!(*previous & mask, 0);
            *previous |= mask;
        }
        replay.output[page as usize * PAGE..page as usize * PAGE + 4].fill(0);
    }
    for (offset, mask) in masks {
        assert_eq!(replay.output[offset], before[offset]);
        replay.output[offset] ^= mask;
    }
    (replay.output, replay.retired)
}

#[test]
#[ignore = "requires eight independently constructed and SDK-qualified repeated retirement views"]
fn repeated_retirement_sources_match_independent_raw_replay_and_current_native_admission() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_REPEATED_RECURSIVE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let recipes = [
        "before",
        "retire-first-leaf",
        "retain-original-tail",
        "exhaust-last-parent",
    ];
    let mut images = Vec::new();
    for profile in profiles() {
        let mut views = Vec::new();
        for (recipe, hash) in recipes.iter().zip(profile.hashes) {
            let raw =
                fs::read(directory.join(format!("h{}-recursive-{recipe}.accdb", profile.header)))
                    .unwrap();
            assert_eq!(raw.len(), profile.length);
            assert_eq!(hex(&Sha256::digest(&raw)), hash);
            views.push(raw);
        }
        images.push((profile, views));
    }

    for (profile, views) in images {
        let before = &views[0];
        let data_pages = indirect(before, profile.table_map, profile.table_bitmap);
        let owned_before = indirect(before, profile.index_map, profile.index_bitmap);
        let global_before = indirect(before, 8123, 513);
        assert_eq!((data_pages.len(), owned_before.len()), (586, 331));
        let originals = rows(before, profile.definition, &data_pages, 798);
        assert_eq!(originals[0].id, 50132);
        assert_eq!(originals[327].id, 148714);
        assert_eq!(originals.last().unwrap().id, 149184);
        let mut original_nodes = BTreeMap::new();
        let original_population = tree(
            before,
            profile.definition,
            profile.root,
            &mut original_nodes,
        );
        assert_eq!(
            original_nodes.keys().copied().collect::<Vec<_>>(),
            owned_before
        );
        assert_eq!(
            original_population,
            originals
                .iter()
                .map(|row| key(row.id, row.reference))
                .collect::<Vec<_>>()
        );
        let root = &original_nodes[&profile.root];
        assert_eq!((root.height, root.prefix, root.tail), (2, 0, 0));
        assert_eq!(root.children, [profile.parent]);
        let parent = &original_nodes[&profile.parent];
        assert_eq!(
            (parent.height, parent.prefix, parent.tail),
            (1, 2, profile.tail)
        );
        assert_eq!(parent.children.len(), 328);
        assert_eq!(original_nodes[&profile.tail].keys.len(), 470);

        for (index, raw) in views.iter().enumerate() {
            let selected = originals
                .iter()
                .filter(|row| match index {
                    0 => false,
                    1 => row.id == 50132,
                    2 => row.id <= 148714,
                    3 => true,
                    _ => unreachable!(),
                })
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(selected.len(), [0, 1, 328, 798][index]);
            let (expected, retired) = reconstruct(before, &profile, &selected, &original_nodes);
            assert_eq!(retired.len(), [0, 1, 328, 330][index]);
            assert_eq!(
                &expected, raw,
                "h{} {}: independent complete raw replay",
                profile.header, recipes[index]
            );
            let live = 798 - selected.len();
            let actual_rows = rows(raw, profile.definition, &data_pages, live);
            let expected_rows = actual_rows
                .iter()
                .map(|row| vec![Value::Int32(row.id), Value::Binary(Vec::new())])
                .collect::<Vec<_>>();
            assert_eq!(
                indirect(raw, profile.table_map, profile.table_bitmap),
                data_pages
            );
            let indexes = indirect(raw, profile.index_map, profile.index_bitmap);
            assert_eq!(
                indexes,
                owned_before
                    .iter()
                    .copied()
                    .filter(|page| !retired.contains(page))
                    .collect::<Vec<_>>()
            );
            assert_eq!(indexes.len(), [331, 330, 3, 1][index]);
            let global = indirect(raw, 8123, 513);
            assert_eq!(
                global.into_iter().collect::<BTreeSet<_>>(),
                global_before
                    .iter()
                    .copied()
                    .chain(retired.iter().copied())
                    .collect::<BTreeSet<_>>()
            );
            for &page in &retired {
                let base = page as usize * PAGE;
                assert_eq!(&raw[base..base + 4], &[0; 4]);
                assert_eq!(&raw[base + 4..base + PAGE], &before[base + 4..base + PAGE]);
            }
            let mut nodes = BTreeMap::new();
            let population = tree(raw, profile.definition, profile.root, &mut nodes);
            assert_eq!(nodes.keys().copied().collect::<Vec<_>>(), indexes);
            assert_eq!(
                population,
                actual_rows
                    .iter()
                    .map(|row| key(row.id, row.reference))
                    .collect::<Vec<_>>()
            );
            let current_root = &nodes[&profile.root];
            if index == 3 {
                assert_eq!(
                    (current_root.height, current_root.prefix, current_root.tail),
                    (0, 0, 0)
                );
                assert!(current_root.keys.is_empty());
                assert!(current_root.children.is_empty());
                assert_eq!(raw[profile.root as usize * PAGE], 4);
                assert_eq!(
                    &raw[profile.root as usize * PAGE + ACTIVE..(profile.root as usize + 1) * PAGE],
                    &before
                        [profile.root as usize * PAGE + ACTIVE..(profile.root as usize + 1) * PAGE]
                );
            } else {
                assert_eq!(
                    (current_root.height, current_root.prefix, current_root.tail),
                    (2, 0, 0)
                );
                assert_eq!(current_root.children, [profile.parent]);
                assert_eq!(current_root.keys, root.keys);
                if index == 1 {
                    assert_eq!(nodes[&profile.next].previous, 0);
                }
                if index == 2 {
                    let parent = &nodes[&profile.parent];
                    assert_eq!((parent.height, parent.prefix, parent.tail), (1, 2, 0));
                    assert_eq!(parent.children, [profile.tail]);
                    assert_eq!(
                        (nodes[&profile.tail].previous, nodes[&profile.tail].next),
                        (0, 0)
                    );
                }
            }
            for (&page, node) in &nodes {
                if node.previous != 0 {
                    assert_eq!(nodes[&node.previous].height, node.height);
                    assert_eq!(nodes[&node.previous].next, page);
                }
                if node.next != 0 {
                    assert_eq!(nodes[&node.next].height, node.height);
                    assert_eq!(nodes[&node.next].previous, page);
                }
            }

            let image =
                NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let catalog = inspect_table_schema(
                &mut image.reader(),
                SCOPE,
                "MSysObjects",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(catalog.columns().len(), 17);
            let source = super::capture(&image, "BinaryStorage", &mut budget(), &|| false).unwrap();
            assert_eq!(source.table.schema().definition_page(), profile.definition);
            assert_eq!(source.table.schema().declared_rows(), live as u32);
            assert_eq!(source.table.rows(), expected_rows);
            assert_eq!(source.records.len(), live);
            for (actual, row) in source.records.iter().zip(&actual_rows) {
                assert_eq!(actual.reference, row.reference);
                assert_eq!(actual.offset, row.offset);
                assert_eq!(actual.data, row.raw);
            }
            let checked = crate::table::indexes::check_table_retained(
                &mut image.reader(),
                SCOPE,
                "BinaryStorage",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(checked.rows(), live);
            assert_eq!(checked.indexes().len(), 1);
            assert_eq!(checked.indexes()[0].status(), IndexCheckStatus::Checked);
            assert_eq!(checked.indexes()[0].index().root_page(), profile.root);
            let history = inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                "BinaryStorage",
                "PrimaryID",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(history.value(), 149184);
            assert_eq!(number(raw, profile.definition as usize * PAGE + 67), 149184);
            let constraints = check_table_constraints(
                &mut image.reader(),
                SCOPE,
                "BinaryStorage",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(constraints.rows(), live);
            assert!(constraints.relationships().is_empty());
            for indexed in [false, true] {
                let actual = if indexed {
                    read_table_index(
                        &mut image.reader(),
                        SCOPE,
                        "BinaryStorage",
                        "PrimaryID",
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
                } else {
                    read_table(
                        &mut image.reader(),
                        SCOPE,
                        "BinaryStorage",
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
                };
                assert_eq!(actual.rows(), expected_rows);
            }
            let sentinel = read_table(
                &mut image.reader(),
                SCOPE,
                "Sentinel",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                sentinel.rows(),
                &[vec![Value::Int32(91), Value::Int32(12345)]]
            );
            assert_eq!(
                hex(&Sha256::digest(image.as_bytes())),
                profile.hashes[index]
            );
        }
    }
}
