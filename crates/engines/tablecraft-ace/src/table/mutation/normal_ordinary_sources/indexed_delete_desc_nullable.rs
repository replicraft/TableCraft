//! Independent indexed DELETE source reads; no DELETE planner, serializer or journal.
use crate::{
    IndexCheckStatus, NativeImage, PayloadPresence, check_table, check_table_constraints,
    inspect_table_index_counter, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{env, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(163);
const ROWS: [[Option<i32>; 3]; 3] = [
    [Some(1), Some(-1), None],
    [Some(2), Some(0), Some(1)],
    [Some(3), Some(1), Some(2)],
];

// Exact independently constructed manifest and literal source inventory; no JSON decoder.
const CASES: [(u8, &str, usize, &str, &str, &str, &str); 8] = [
    (
        5,
        "null-first",
        389120,
        "h5-desc-null-delete-null-first-before.accdb",
        "f8f4b039d9a692a0e1f9b0ef9ee38201f60508d654705c125275a7a169ae414b",
        "h5-desc-null-delete-null-first-deleted.accdb",
        "5ac960784298110151a18068598ed010179241be76e96aea12a69d5fdc5a688e",
    ),
    (
        5,
        "present-middle",
        389120,
        "h5-desc-null-delete-present-middle-before.accdb",
        "f8f4b039d9a692a0e1f9b0ef9ee38201f60508d654705c125275a7a169ae414b",
        "h5-desc-null-delete-present-middle-deleted.accdb",
        "eec3009356de6fca540fae4c6b48188fcaa4a3a7581d13f03092a671efe83f20",
    ),
    (
        5,
        "exterior",
        389120,
        "h5-desc-null-delete-exterior-before.accdb",
        "f8f4b039d9a692a0e1f9b0ef9ee38201f60508d654705c125275a7a169ae414b",
        "h5-desc-null-delete-exterior-deleted.accdb",
        "16d90236e802bb484187876f90aa7e1ca1fd97807818a4b970efb3348b890a0b",
    ),
    (
        5,
        "all",
        389120,
        "h5-desc-null-delete-all-before.accdb",
        "f8f4b039d9a692a0e1f9b0ef9ee38201f60508d654705c125275a7a169ae414b",
        "h5-desc-null-delete-all-deleted.accdb",
        "87a71e52cd0d5d0bb5ae1d13a4140b9cc715a57a0d6e83f4fadaa6da85669a2d",
    ),
    (
        6,
        "null-first",
        458752,
        "h6-desc-null-delete-null-first-before.accdb",
        "c6d5f83770a25c8b6d0f34bdaab729d4d181f37fc738c47f5db9339d895fddae",
        "h6-desc-null-delete-null-first-deleted.accdb",
        "60aa0a895fc4e7f13fb1c6c492eee5796bc0b67705b2797f38b2c1ad7ea8671a",
    ),
    (
        6,
        "present-middle",
        458752,
        "h6-desc-null-delete-present-middle-before.accdb",
        "c6d5f83770a25c8b6d0f34bdaab729d4d181f37fc738c47f5db9339d895fddae",
        "h6-desc-null-delete-present-middle-deleted.accdb",
        "d7e88b430f11694d1675e93b2e26c54e5baa1975013cb3a9c40b9735e14a8eaf",
    ),
    (
        6,
        "exterior",
        458752,
        "h6-desc-null-delete-exterior-before.accdb",
        "c6d5f83770a25c8b6d0f34bdaab729d4d181f37fc738c47f5db9339d895fddae",
        "h6-desc-null-delete-exterior-deleted.accdb",
        "8d4e8a882078fea639f09cba9e340ae36ffa90f23c88946fedf17da4d4a80ab5",
    ),
    (
        6,
        "all",
        458752,
        "h6-desc-null-delete-all-before.accdb",
        "c6d5f83770a25c8b6d0f34bdaab729d4d181f37fc738c47f5db9339d895fddae",
        "h6-desc-null-delete-all-deleted.accdb",
        "0e226ffebf9de008b61620175cc0a65654fc19ff6325632abd87f10bcc02ceea",
    ),
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn entry(value: Option<i32>, data: u32, slot: usize, descending: bool) -> Vec<u8> {
    let mut raw = Vec::new();
    match value {
        None => raw.push(if descending { 255 } else { 0 }),
        Some(value) => {
            let biased = (value as u32) ^ 0x8000_0000;
            raw.push(if descending { 128 } else { 127 });
            let encoded = if descending { !biased } else { biased };
            raw.extend_from_slice(&encoded.to_be_bytes());
        }
    }
    raw.extend_from_slice(&((data << 8) | slot as u32).to_be_bytes());
    raw
}

fn ordered(retained: &[usize], physical: usize) -> Vec<usize> {
    let source = if physical == 0 { [0, 1, 2] } else { [2, 1, 0] };
    source
        .into_iter()
        .filter(|slot| retained.contains(slot))
        .collect()
}

fn values(slots: &[usize]) -> Vec<Vec<Value>> {
    slots
        .iter()
        .map(|slot| {
            ROWS[*slot]
                .into_iter()
                .map(|value| match value {
                    Some(value) => Value::Int32(value),
                    None => Value::Null,
                })
                .collect()
        })
        .collect()
}

fn entry_bitmap(entries: &[Vec<u8>]) -> [u8; 453] {
    let mut bitmap = [0; 453];
    let mut end = 0;
    for entry in entries {
        end += entry.len();
        bitmap[end / 8] |= 1 << (end % 8);
    }
    bitmap
}

fn read_view(bytes: &[u8], definition: u32, maps: u32, roots: [u32; 2], retained: &[usize]) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(table.schema().definition_page(), definition);
    assert_eq!(table.schema().declared_rows() as usize, retained.len());
    assert_eq!(table.columns().len(), 3);
    for (field, name) in table
        .schema()
        .columns()
        .iter()
        .zip(["Id", "Score", "Guard"])
    {
        assert_eq!(field.name(), name);
        assert_eq!(
            (
                field.native_type(),
                field.flags(),
                field.extended_flags(),
                field.storage_length()
            ),
            (4, 3, 0, 4)
        );
    }
    let expected = values(retained);
    assert_eq!(table.rows(), expected);
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), retained.len());
    assert_eq!(checked.indexes().len(), 4);
    for (checked, (name, physical, column, kind, flags)) in checked.indexes().iter().zip([
        ("PrimaryID", 0, 0, 1, 137),
        ("PrimaryAlias", 0, 0, 0, 137),
        ("GroupOrder", 1, 2, 0, 128),
        ("GroupAlias", 1, 2, 0, 128),
    ]) {
        assert_eq!(checked.status(), IndexCheckStatus::Checked);
        let index = checked.index();
        assert_eq!(index.name(), name);
        assert_eq!(index.data_number(), physical);
        assert_eq!(index.root_page(), roots[physical as usize]);
        assert_eq!(index.usage_map(), (maps << 8) | (2 + physical));
        assert_eq!((index.native_type(), index.flags()), (kind, flags));
        assert_eq!(index.columns().len(), 1);
        assert_eq!(index.columns()[0].number(), column);
        assert_eq!(index.columns()[0].descending(), physical == 1);
        let counter = inspect_table_index_counter(
            &mut image.reader(),
            SCOPE,
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(counter.physical_index(), physical);
        assert_eq!(counter.value(), 3);
        let indexed = read_table_index(
            &mut image.reader(),
            SCOPE,
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let order = ordered(retained, physical as usize);
        assert_eq!(indexed.rows(), values(&order));
    }
    let constraints =
        check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(constraints.rows(), retained.len());
    let coverage = image
        .inspect_table_macro_coverage(table.schema().object_id(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(coverage.body().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.module().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.table_extra().presence(), PayloadPresence::Absent);
    assert!(coverage.children().is_empty());
    image
        .check_native_ordinary_fixed_table("Items", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
#[ignore = "requires independently constructed indexed DELETE source directory"]
fn genuine_desc_nullable_delete_literals_read_all_aliases_and_protect_complete_complement() {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_DESC_NULL_DELETE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let manifest = fs::read(directory.join("manifest.json")).unwrap();
    assert_eq!(
        hash(&manifest),
        "5cb3b6ec5efe19b7d1f73cfa6090ae5b57cc6cc489880cd0edda574ca8eb353c"
    );
    let mut seen = Vec::new();
    for (header, recipe, bytes, before_file, before_hash, after_file, after_hash) in CASES {
        let (definition, data, maps, roots, original_hash) = match header {
            5 => (
                89,
                93,
                90,
                [91, 92],
                "f8f4b039d9a692a0e1f9b0ef9ee38201f60508d654705c125275a7a169ae414b",
            ),
            6 => (
                106,
                110,
                107,
                [108, 109],
                "c6d5f83770a25c8b6d0f34bdaab729d4d181f37fc738c47f5db9339d895fddae",
            ),
            _ => panic!("unexpected header"),
        };
        let selected: &[usize] = match recipe {
            "null-first" => &[0],
            "present-middle" => &[1],
            "exterior" => &[0, 2],
            "all" => &[0, 1, 2],
            _ => panic!("unexpected literal recipe"),
        };
        let expected_orders: &[&[usize]] = match recipe {
            "null-first" => &[&[0]],
            "present-middle" => &[&[1]],
            "exterior" => &[&[0, 2], &[2, 0]],
            "all" => &[&[0, 1, 2]],
            _ => unreachable!(),
        };
        // The pinned whole manifest contains these exact original request-order
        // laws. Normalize each literal order independently; never call a planner.
        for order in expected_orders {
            let mut normalized = order.to_vec();
            normalized.sort();
            assert_eq!(normalized, selected);
        }
        seen.push((header, recipe.to_owned()));
        let retained = (0..3)
            .filter(|slot| !selected.contains(slot))
            .collect::<Vec<_>>();
        let load = |file: &str, expected_hash: &str| {
            let raw = fs::read(directory.join(file)).unwrap();
            assert_eq!(raw.len(), bytes);
            assert_eq!(hash(&raw), expected_hash);
            raw
        };
        assert_eq!(before_hash, original_hash);
        let before = load(before_file, before_hash);
        let after = load(after_file, after_hash);
        assert_eq!(hash(&before), original_hash);
        assert_eq!(before.len(), after.len());
        read_view(&before, definition, maps, roots, &[0, 1, 2]);
        read_view(&after, definition, maps, roots, &retained);
        let td = definition as usize * 4096;
        let base = data as usize * 4096;
        let mut expected = before.clone();
        assert_eq!(&before[td + 16..td + 20], &[3, 0, 0, 0]);
        expected[td + 16..td + 20].copy_from_slice(&(retained.len() as u32).to_le_bytes());
        for (slot, row) in ROWS.iter().enumerate() {
            let start = 4081 - slot * 15;
            let mut record = vec![3, 0];
            for &value in row {
                // Observed inactive Guard payload; not a logical value.
                let payload = value.unwrap_or_default();
                record.extend_from_slice(&payload.to_le_bytes());
            }
            record.push(if slot == 0 { 3 } else { 7 });
            assert_eq!(&before[base + start..base + start + 15], record);
            let original = start as u16;
            let at = base + 14 + slot * 2;
            assert_eq!(&before[at..at + 2], original.to_le_bytes());
            if selected.contains(&slot) {
                expected[at..at + 2].copy_from_slice(&(original | 0xc000).to_le_bytes());
            }
        }
        for (physical, root) in roots.into_iter().enumerate() {
            let page = root as usize * 4096;
            let column = if physical == 0 { 0 } else { 2 };
            assert_eq!(&before[page..page + 2], &[4, 1]);
            let original_free = if physical == 0 { 3589_u16 } else { 3593_u16 };
            assert_eq!(&before[page + 2..page + 4], &original_free.to_le_bytes());
            assert_eq!(&before[page + 4..page + 8], &definition.to_le_bytes());
            assert_eq!(&before[page + 8..page + 27], &[0; 19]);
            let original_entries = ordered(&[0, 1, 2], physical)
                .into_iter()
                .map(|slot| entry(ROWS[slot][column], data, slot, physical == 1))
                .collect::<Vec<_>>();
            let original_keys = original_entries.concat();
            let original_lengths = original_entries.iter().map(Vec::len).collect::<Vec<_>>();
            assert_eq!(
                original_lengths,
                if physical == 0 {
                    vec![9, 9, 9]
                } else {
                    vec![9, 9, 5]
                }
            );
            assert_eq!(
                &before[page + 480..page + 480 + original_keys.len()],
                original_keys
            );
            assert_eq!(
                &before[page + 27..page + 480],
                &entry_bitmap(&original_entries)
            );
            let entries = ordered(&retained, physical)
                .into_iter()
                .map(|slot| entry(ROWS[slot][column], data, slot, physical == 1))
                .collect::<Vec<_>>();
            let keys = entries.concat();
            let expected_active = match (recipe, physical) {
                ("null-first", _) | ("present-middle", 0) => 18,
                ("present-middle", 1) => 14,
                ("exterior", _) => 9,
                ("all", _) => 0,
                _ => panic!("unexpected source profile"),
            };
            assert_eq!(keys.len(), expected_active);
            expected[page + 2..page + 4]
                .copy_from_slice(&((3616 - keys.len()) as u16).to_le_bytes());
            expected[page + 27..page + 480].copy_from_slice(&entry_bitmap(&entries));
            expected[page + 480..page + 480 + keys.len()].copy_from_slice(&keys);
            assert_eq!(
                &after[page + 480 + keys.len()..page + 4096],
                &before[page + 480 + keys.len()..page + 4096]
            );
        }
        assert_eq!(after, expected, "complete literal indexed DELETE image");
    }
    seen.sort();
    let mut expected = Vec::new();
    for header in [5, 6] {
        for recipe in ["all", "exterior", "null-first", "present-middle"] {
            expected.push((header, recipe.to_owned()));
        }
    }
    assert_eq!(seen, expected);
}
