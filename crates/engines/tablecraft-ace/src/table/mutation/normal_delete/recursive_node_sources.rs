//! Original-only current native admission. No planner, AFTER or candidate call.
use crate::{
    IndexCheckStatus, NativeImage, check_table_constraints, inspect_table_index_counter,
    inspect_table_schema, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;
const ACTIVE: usize = 480;
const MASK: usize = 27;
const MASK_BYTES: usize = 453;
const SCOPE: CatalogScope = CatalogScope::new(263);

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

fn rows(raw: &[u8], definition: u32, owned: &[u32]) -> Vec<Row> {
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
    assert_eq!((slots, deleted, result.len()), (149184, 148221, 963));
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
    assert!(!entries.is_empty());
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

#[test]
#[ignore = "requires the two immutable height-two SDK recursive INSERT originals"]
fn recursive_node_originals_require_current_native17_source_constraints_and_complete_owned_tree() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_RECURSIVE_NODE_ORIGINAL_DIR").unwrap());
    assert!(directory.is_absolute());
    for (
        profile,
        length,
        definition,
        root,
        parent,
        other,
        leaf,
        previous_leaf,
        next_leaf,
        table_map,
        index_map,
        table_bitmap,
        index_bitmap,
        wanted_hash,
        first_bound,
        next_bound,
        last_bound,
    ) in [
        (
            "V2016",
            4837376,
            89,
            91,
            821,
            820,
            459,
            457,
            461,
            372667,
            372529,
            97,
            605,
            "241e192b746f0f8b88e787d5b58407983fea37cacc8a997f7ccbb56377a95b71",
            "7f8000c2a60001c868",
            "7f8000c1780001c639",
            "7f800246c000049c08",
        ),
        (
            "V2019",
            4911104,
            106,
            108,
            839,
            838,
            477,
            475,
            479,
            442299,
            442161,
            115,
            621,
            "c3fc4d82f997163d764e5d8cd13f4362b6169167094b4c791d32d817e83065b7",
            "7f8000c2a60001da68",
            "7f8000c1780001d839",
            "7f800246c00004ae08",
        ),
    ] {
        let path = directory.join(format!("{profile}-internal-source.accdb"));
        let raw = fs::read(path).unwrap();
        assert_eq!(raw.len(), length);
        assert_eq!(hex(&Sha256::digest(&raw)), wanted_hash);
        let image =
            NativeImage::capture(&mut Cursor::new(&raw), SCOPE, &mut budget(), &|| false).unwrap();
        let catalog = inspect_table_schema(
            &mut image.reader(),
            SCOPE,
            "MSysObjects",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            catalog.columns().len(),
            17,
            "{profile}: no alternate catalog profile"
        );
        let source = super::capture(&image, "BinaryStorage", &mut budget(), &|| false)
            .unwrap_or_else(|error| {
                panic!("{profile}: current full NoRequired/Source admission refused: {error:?}")
            });
        let expected_owned = indirect(&raw, table_map, table_bitmap);
        let expected_indexes = indirect(&raw, index_map, index_bitmap);
        let global = indirect(&raw, 8123, 513);
        assert_eq!((expected_owned.len(), expected_indexes.len()), (586, 497));
        assert!(expected_owned.iter().all(|page| !global.contains(page)));
        assert!(expected_indexes.iter().all(|page| !global.contains(page)));
        assert!(
            expected_owned
                .iter()
                .all(|page| !expected_indexes.contains(page))
        );
        let original = rows(&raw, definition, &expected_owned);
        let expected_rows = original
            .iter()
            .map(|row| vec![Value::Int32(row.id), Value::Binary(Vec::new())])
            .collect::<Vec<_>>();
        assert_eq!(source.table.rows(), expected_rows);
        assert_eq!(source.table.schema().definition_page(), definition);
        assert_eq!(source.table.schema().declared_rows(), 963);
        assert_eq!(source.records.len(), 963);
        assert_eq!(number(&raw, definition as usize * PAGE + 16), 963);
        assert_eq!(original[164].id, 49830);
        assert_eq!(original[164].raw, record(49830));
        assert_eq!(
            original[..165].iter().map(|row| row.id).collect::<Vec<_>>(),
            (1..=165).map(|position| position * 302).collect::<Vec<_>>()
        );
        let (owned_cell, global_cell, bit) = if profile == "V2016" {
            (2478141, 2101309, 8)
        } else {
            (2543679, 2101311, 32)
        };
        assert_ne!(raw[owned_cell] & bit, 0);
        assert_eq!(raw[global_cell] & bit, 0);
        for (observed, expected) in source.records.iter().zip(&original) {
            assert_eq!(observed.reference, expected.reference);
            assert_eq!(observed.offset, expected.offset);
            assert_eq!(observed.data, expected.raw);
        }
        let mut nodes = BTreeMap::new();
        let population = tree(&raw, definition, root, &mut nodes);
        assert_eq!(nodes.keys().copied().collect::<Vec<_>>(), expected_indexes);
        assert_eq!(
            population,
            original
                .iter()
                .map(|row| key(row.id, row.reference))
                .collect::<Vec<_>>()
        );
        assert_eq!(nodes[&root].height, 2);
        assert_eq!(nodes[&root].prefix, 0);
        assert_eq!(nodes[&root].tail, 0);
        assert_eq!(nodes[&root].children, [parent, other]);
        assert_eq!(hex(&nodes[&root].keys[0]), first_bound);
        assert_eq!(hex(&nodes[&root].keys[1]), last_bound);
        assert_eq!(nodes[&parent].children.len(), 164);
        assert_eq!(nodes[&parent].tail, leaf);
        assert_eq!(nodes[&other].children.len(), 328);
        assert_ne!(nodes[&other].tail, 0);
        assert_eq!(hex(nodes[&previous_leaf].keys.last().unwrap()), next_bound);
        assert_eq!(
            (nodes[&leaf].previous, nodes[&leaf].next),
            (previous_leaf, next_leaf)
        );
        assert_eq!(nodes[&parent].next, other);
        assert_eq!(nodes[&other].previous, parent);
        for height in 0..=2 {
            let mut level = nodes
                .iter()
                .filter(|(_, node)| node.height == height)
                .map(|(&number, _)| number)
                .collect::<Vec<_>>();
            let first = *level
                .iter()
                .find(|&&number| nodes[&number].previous == 0)
                .unwrap();
            let mut chain = Vec::new();
            let mut current = first;
            let mut previous = 0;
            while current != 0 {
                assert!(!chain.contains(&current));
                assert_eq!(nodes[&current].previous, previous);
                chain.push(current);
                previous = current;
                current = nodes[&current].next;
            }
            level.sort_unstable();
            chain.sort_unstable();
            assert_eq!(chain, level);
        }
        let checked = crate::table::indexes::check_table_retained(
            &mut image.reader(),
            SCOPE,
            "BinaryStorage",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), 963);
        assert_eq!(checked.indexes().len(), 1);
        assert_eq!(checked.indexes()[0].status(), IndexCheckStatus::Checked);
        assert_eq!(checked.indexes()[0].index().name(), "PrimaryID");
        assert_eq!(checked.indexes()[0].index().root_page(), root);
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
        assert_eq!(number(&raw, definition as usize * PAGE + 67), 149184);
        let constraints = check_table_constraints(
            &mut image.reader(),
            SCOPE,
            "BinaryStorage",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(constraints.rows(), 963);
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
        assert_eq!(hex(&Sha256::digest(image.as_bytes())), wanted_hash);
    }
}
