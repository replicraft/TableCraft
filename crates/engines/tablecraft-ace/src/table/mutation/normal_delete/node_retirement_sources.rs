//! Literal NODE retirement source qualification; no planner or candidate call.
use crate::{
    IndexCheckStatus, NativeImage, check_table_constraints, inspect_table_index_counter,
    read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(219);
const ACTIVE: usize = 480;
const MASK: usize = 27;
const MASK_BYTES: usize = 453;

struct OriginalRow {
    directory: usize,
    reference: u32,
    word: u16,
    values: [i32; 3],
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn word(raw: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(raw[at..at + 2].try_into().unwrap())
}

fn number(raw: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(raw[at..at + 4].try_into().unwrap())
}

fn original_rows(raw: &[u8], definition: u32) -> Vec<OriginalRow> {
    let mut rows = Vec::new();
    for (page_number, page) in raw.chunks_exact(PAGE).enumerate() {
        if page[..2] != [1, 1] || number(page, 4) != definition {
            continue;
        }
        let count = word(page, 12) as usize;
        assert!(count <= 255);
        let mut end = PAGE;
        for slot in 0..count {
            let directory = page_number * PAGE + 14 + 2 * slot;
            let start = word(raw, directory);
            assert_eq!(start & 0xe000, 0);
            assert!(14 + 2 * count <= usize::from(start));
            let record = &page[usize::from(start)..end];
            assert_eq!(record.len(), 15);
            assert_eq!(&record[..2], &[3, 0]);
            assert_eq!(record[14], 7);
            let values = std::array::from_fn(|index| {
                i32::from_le_bytes(record[2 + 4 * index..6 + 4 * index].try_into().unwrap())
            });
            let id = rows.len() as i32 + 1;
            assert_eq!(values, [id, id - 401, (id - 1) % 7]);
            rows.push(OriginalRow {
                directory,
                reference: (page_number as u32) << 8 | slot as u32,
                word: start,
                values,
            });
            end = usize::from(start);
        }
    }
    assert_eq!(rows.len(), 800);
    rows
}

fn selected(recipe: &str, identity: i32) -> bool {
    match recipe {
        "before" => false,
        "retire-left" => identity <= 302,
        "retire-right" => identity >= 303,
        "delete-all" => true,
        _ => panic!("outside frozen retirement requests"),
    }
}

fn entries(before: &[u8], page: u32) -> Vec<Vec<u8>> {
    let raw = &before[page as usize * PAGE..(page as usize + 1) * PAGE];
    assert_eq!(&raw[..2], &[4, 1]);
    let prefix = usize::from(word(raw, 24));
    let mut keys: Vec<Vec<u8>> = Vec::new();
    let mut end = 0;
    for bit in 0..MASK_BYTES * 8 {
        if raw[MASK + bit / 8] & (1 << (bit % 8)) == 0 {
            continue;
        }
        assert!(end < bit && ACTIVE + bit <= PAGE);
        let mut key = if keys.is_empty() {
            Vec::new()
        } else {
            keys[0][..prefix].to_vec()
        };
        key.extend_from_slice(&raw[ACTIVE + end..ACTIVE + bit]);
        assert_eq!(key.len(), 9);
        assert_eq!(key[0], 127);
        keys.push(key);
        end = bit;
    }
    assert_eq!(usize::from(word(raw, 2)), PAGE - ACTIVE - end);
    keys
}

fn write_page(before: &[u8], after: &mut [u8], page: u32, entries: &[Vec<u8>]) {
    let base = page as usize * PAGE;
    let mut common = entries.first().map_or(0, Vec::len);
    for entry in entries.iter().skip(1) {
        common = common.min(entry.len());
        common = (0..common)
            .find(|index| entries[0][*index] != entry[*index])
            .unwrap_or(common);
    }
    let prefix = usize::from(word(before, base + 24)).min(common);
    let mut mask = [0; MASK_BYTES];
    let mut active = Vec::new();
    for (position, entry) in entries.iter().enumerate() {
        active.extend_from_slice(if position == 0 {
            entry
        } else {
            &entry[prefix..]
        });
        let end = active.len();
        assert!(0 < end && end <= PAGE - ACTIVE);
        mask[end / 8] |= 1 << (end % 8);
    }
    after[base + 2..base + 4]
        .copy_from_slice(&((PAGE - ACTIVE - active.len()) as u16).to_le_bytes());
    after[base + 24..base + 26].copy_from_slice(&(prefix as u16).to_le_bytes());
    after[base + MASK..base + ACTIVE].copy_from_slice(&mask);
    after[base + ACTIVE..base + ACTIVE + active.len()].copy_from_slice(&active);
}

fn literal(
    before: &[u8],
    definition: u32,
    roots: [u32; 2],
    children: [[u32; 2]; 2],
    owned: [usize; 2],
    recipe: &str,
) -> Vec<u8> {
    if recipe == "before" {
        return before.to_vec();
    }
    let rows = original_rows(before, definition);
    let removed = rows
        .iter()
        .filter(|row| selected(recipe, row.values[0]))
        .map(|row| row.reference)
        .collect::<Vec<_>>();
    let mut after = before.to_vec();
    let count = definition as usize * PAGE + 16;
    after[count..count + 4].copy_from_slice(&((800 - removed.len()) as u32).to_le_bytes());
    for row in rows.iter().filter(|row| removed.contains(&row.reference)) {
        after[row.directory..row.directory + 2].copy_from_slice(&(row.word | 0xc000).to_le_bytes());
    }
    for physical in 0..2 {
        let mut kept = Vec::new();
        for child in children[physical] {
            let surviving = entries(before, child)
                .into_iter()
                .filter(|key| {
                    let reference = u32::from_be_bytes(key[5..9].try_into().unwrap());
                    !removed.contains(&reference)
                })
                .collect::<Vec<_>>();
            if surviving.is_empty() {
                let base = child as usize * PAGE;
                after[base..base + 4].fill(0);
                let map = owned[physical];
                assert_eq!(before[map], 0);
                let first = number(before, map + 1);
                assert!(child >= first && child - first < 512);
                let delta = (child - first) as usize;
                let cell = map + 5 + delta / 8;
                let mask = 1 << (delta % 8);
                assert_ne!(before[cell] & mask, 0);
                after[cell] &= !mask;
                // Frozen GLOBAL row 1:0 occupies 8123..8192, inline base 0.
                assert_eq!(&before[8123..8128], &[0, 0, 0, 0, 0]);
                let global = 8123 + 5 + child as usize / 8;
                let mask = 1 << (child % 8);
                assert_eq!(before[global] & mask, 0);
                after[global] |= mask;
            } else {
                kept.push((child, surviving));
            }
        }
        for (position, (child, population)) in kept.iter().enumerate() {
            write_page(before, &mut after, *child, population);
            let previous = position.checked_sub(1).map_or(0, |index| kept[index].0);
            let next = kept.get(position + 1).map_or(0, |entry| entry.0);
            let base = *child as usize * PAGE;
            after[base + 12..base + 16].copy_from_slice(&previous.to_le_bytes());
            after[base + 16..base + 20].copy_from_slice(&next.to_le_bytes());
        }
        let root = roots[physical];
        let mut bounds = Vec::new();
        for (position, (child, population)) in kept.iter().enumerate() {
            if kept.len() > 1 && position + 1 == kept.len() {
                continue;
            }
            let mut bound = population.last().unwrap().clone();
            bound.extend_from_slice(&child.to_be_bytes());
            bounds.push(bound);
        }
        write_page(before, &mut after, root, &bounds);
        let base = root as usize * PAGE;
        let tail = if kept.len() > 1 {
            kept.last().unwrap().0
        } else {
            0
        };
        after[base + 20..base + 24].copy_from_slice(&tail.to_le_bytes());
        if kept.is_empty() {
            after[base] = 4;
            after[base + 26] = 0;
        }
    }
    after
}

#[test]
#[ignore = "requires eight independently constructed and SDK-qualified retirement views"]
fn node_child_retirement_sources_preserve_residuals_maps_history_aliases_and_full_rows() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_NODE_RETIREMENT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut views = 0;
    for (header, recipe, wanted_hash) in [
        (
            5_u8,
            "before",
            "0d9cf91fff6c9153c628bf5cd54dc39a9420b18f4fad669fca8e9a3c8a92694b",
        ),
        (
            5_u8,
            "retire-left",
            "2d8d8592a86e4339885fad6ad3222a0f3bbce95cfca99233b12849af212d2b79",
        ),
        (
            5_u8,
            "retire-right",
            "007e07984df6d4e49cee492b2f8c793570823e0327f68e94dbdd9429cebef5fe",
        ),
        (
            5_u8,
            "delete-all",
            "51a8d030a309648e91945d76144db78c7c8ab574f0d8af7bc579adbae77e16a4",
        ),
        (
            6_u8,
            "before",
            "18eff74333ed55f4bfef879d6a2530fab8031aaef6a07895f50aec13fdf079ae",
        ),
        (
            6_u8,
            "retire-left",
            "702425eaf4886321522ac743d8af251b351fa55748524de4f8631cc4149ccbf2",
        ),
        (
            6_u8,
            "retire-right",
            "ce808f9787fe418f16f14e317c463ad06c10a9659e623244077751bc99c33951",
        ),
        (
            6_u8,
            "delete-all",
            "f1329cc6ce17f1adfb2702f54e1c9a4d02442772e6505b09a92a72c41287230a",
        ),
    ] {
        let (definition, roots, children, owned) = if header == 5 {
            (89, [91, 92], [[98, 97], [101, 100]], [372529, 372460])
        } else {
            (106, [108, 109], [[115, 114], [118, 117]], [442161, 442092])
        };
        let before = fs::read(directory.join(format!("h{header}-node-before.accdb"))).unwrap();
        assert_eq!(
            digest(&before),
            if header == 5 {
                "0d9cf91fff6c9153c628bf5cd54dc39a9420b18f4fad669fca8e9a3c8a92694b"
            } else {
                "18eff74333ed55f4bfef879d6a2530fab8031aaef6a07895f50aec13fdf079ae"
            }
        );
        let raw = fs::read(directory.join(format!("h{header}-node-{recipe}.accdb"))).unwrap();
        assert_eq!(digest(&raw), wanted_hash);
        assert_eq!(
            raw,
            literal(&before, definition, roots, children, owned, recipe)
        );
        let original = original_rows(&before, definition);
        let survivors = original
            .iter()
            .filter(|row| !selected(recipe, row.values[0]))
            .collect::<Vec<_>>();
        let expected = survivors
            .iter()
            .map(|row| row.values.into_iter().map(Value::Int32).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let image =
            NativeImage::capture(&mut Cursor::new(&raw), SCOPE, &mut budget(), &|| false).unwrap();
        let source = crate::table::mutation::normal_actions::capture(
            &image,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(source.table.rows(), expected);
        assert_eq!(source.records.len(), survivors.len());
        for (actual, wanted) in source.records.iter().zip(&survivors) {
            assert_eq!(actual.reference, wanted.reference);
            let start = (wanted.reference >> 8) as usize * PAGE + usize::from(wanted.word);
            assert_eq!(actual.data, before[start..start + 15]);
        }
        let constraints =
            check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(constraints.rows(), expected.len());
        assert!(constraints.relationships().is_empty());
        let checked = crate::table::indexes::check_table_retained(
            &mut image.reader(),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), expected.len());
        assert_eq!(checked.indexes().len(), 4);
        for (name, physical, column, history) in [
            ("PrimaryID", 0, 0, 800),
            ("PrimaryAlias", 0, 0, 800),
            ("GroupOrder", 1, 2, 7),
            ("GroupAlias", 1, 2, 7),
        ] {
            let index = checked
                .indexes()
                .iter()
                .find(|index| index.index().name() == name)
                .unwrap();
            assert_eq!(index.status(), IndexCheckStatus::Checked);
            assert_eq!(index.index().root_page(), roots[physical]);
            assert_eq!(index.index().data_number() as usize, physical);
            let mut ordered = survivors.clone();
            ordered.sort_by_key(|row| (row.values[column], row.reference));
            let wanted = ordered
                .iter()
                .map(|row| row.values.into_iter().map(Value::Int32).collect::<Vec<_>>())
                .collect::<Vec<_>>();
            let cursor = read_table_index(
                &mut image.reader(),
                SCOPE,
                "Items",
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(cursor.rows(), wanted);
            let counter = inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                "Items",
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(counter.value(), history);
        }
        assert_eq!(image.as_bytes(), raw);
        views += 1;
    }
    assert_eq!(views, 8);
}
