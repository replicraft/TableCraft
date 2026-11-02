//! Independent full-image replay and read qualification; no mutation planner.
use crate::{
    IndexCheckStatus, NativeImage, check_table_constraints,
    format::{allocation, reader::Reader},
    inspect_table_index_counter, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(288);

struct Case {
    header: u8,
    money: bool,
    table: &'static str,
    before: &'static str,
    before_hash: &'static str,
    after: &'static str,
    after_hash: &'static str,
    definition: u32,
    roots: [u32; 3],
    data: [u32; 3],
    eof: u32,
    claims: &'static [(usize, u8, u8, bool)],
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 10_000_000, 2_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_u16(raw: &mut [u8], at: usize, value: u16) {
    raw[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(raw: &mut [u8], at: usize, value: u32) {
    raw[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn reference(case: &Case, ordinal: usize) -> u32 {
    let group = if ordinal < 194 {
        0
    } else if ordinal < 388 {
        1
    } else {
        2
    };
    (case.data[group] << 8) | (ordinal - [0, 194, 388][group]) as u32
}

fn key(units: i64, row: u32, physical: usize) -> Vec<u8> {
    let mut encoded = if physical == 0 {
        ((units as u32) ^ 0x8000_0000).to_be_bytes().to_vec()
    } else {
        ((units as u64) ^ 0x8000_0000_0000_0000)
            .to_be_bytes()
            .to_vec()
    };
    if physical == 2 {
        for byte in &mut encoded {
            *byte = !*byte;
        }
    }
    let mut result = vec![if physical == 2 { 0x80 } else { 0x7f }];
    result.extend_from_slice(&encoded);
    result.extend_from_slice(&row.to_be_bytes());
    result
}

fn population(case: &Case, physical: usize, inserted: bool) -> Vec<Vec<u8>> {
    let mut result: Vec<_> = (0..400)
        .map(|ordinal| {
            key(
                if physical == 0 { ordinal as i64 + 1 } else { 0 },
                reference(case, ordinal),
                physical,
            )
        })
        .collect();
    if inserted {
        result.push(key(
            if physical == 0 { 401 } else { i64::MAX },
            reference(case, 400),
            physical,
        ));
    }
    result.sort();
    result
}

fn frame(seed: &[u8], entries: &[Vec<u8>], prefix: usize) -> Vec<u8> {
    let mut page = seed.to_vec();
    page[27..480].fill(0);
    let mut cursor = 0;
    for (ordinal, entry) in entries.iter().enumerate() {
        let shared = if ordinal == 0 { 0 } else { prefix };
        assert_eq!(&entry[..shared], &entries[0][..shared]);
        let end = cursor + entry.len() - shared;
        assert!(end < 3616);
        page[480 + cursor..480 + end].copy_from_slice(&entry[shared..]);
        page[27 + end / 8] |= 1 << (end % 8);
        cursor = end;
    }
    write_u16(&mut page, 2, (3616 - cursor) as u16);
    write_u16(&mut page, 24, prefix as u16);
    page
}

fn leaf(case: &Case, previous: u32, next: u32, entries: &[Vec<u8>]) -> Vec<u8> {
    let mut page = vec![0; 4096];
    page[..2].copy_from_slice(&[4, 1]);
    write_u32(&mut page, 4, case.definition);
    write_u32(&mut page, 12, previous);
    write_u32(&mut page, 16, next);
    frame(&page, entries, 0)
}

fn replay(case: &Case, before: &[u8]) -> Vec<u8> {
    assert_eq!(before.len(), case.eof as usize * 4096);
    assert_eq!(before[20], case.header);
    let mut after = before.to_vec();
    let td = case.definition as usize * 4096;
    assert_eq!(&before[td + 16..td + 20], &400_u32.to_le_bytes());
    write_u32(&mut after, td + 16, 401);
    for (physical, old) in [400_u32, 1, 1].into_iter().enumerate() {
        let at = td + 67 + physical * 12;
        assert_eq!(&before[at..at + 4], &old.to_le_bytes());
        write_u32(&mut after, at, old + 1);
    }

    let data = case.data[2] as usize * 4096;
    assert_eq!(&before[data + 2..data + 4], &3830_u16.to_le_bytes());
    assert_eq!(&before[data + 12..data + 14], &12_u16.to_le_bytes());
    let mut row = vec![3, 0];
    row.extend_from_slice(&401_i32.to_le_bytes());
    row.extend_from_slice(&i64::MAX.to_le_bytes());
    row.extend_from_slice(&6817_i32.to_le_bytes());
    row.push(7);
    assert_eq!(row.len(), 19);
    after[data + 3849..data + 3868].copy_from_slice(&row);
    write_u16(&mut after, data + 38, 3849);
    write_u16(&mut after, data + 12, 13);
    write_u16(&mut after, data + 2, 3809);

    let primary = case.roots[0] as usize * 4096;
    let original = frame(
        &before[primary..primary + 4096],
        &population(case, 0, false),
        0,
    );
    assert_eq!(&original, &before[primary..primary + 4096]);
    let rewritten = frame(
        &before[primary..primary + 4096],
        &population(case, 0, true),
        0,
    );
    after[primary..primary + 4096].copy_from_slice(&rewritten);
    for physical in 1..3 {
        let root = case.roots[physical] as usize * 4096;
        let original = frame(
            &before[root..root + 4096],
            &population(case, physical, false),
            9,
        );
        assert_eq!(&original, &before[root..root + 4096]);
        let keys = population(case, physical, true);
        let right = case.eof + ((physical - 1) * 2) as u32;
        let left = right + 1;
        let mut parent = before[root..root + 4096].to_vec();
        parent[0] = 3;
        parent[26] = 1;
        write_u32(&mut parent, 20, right);
        let mut fence = keys[199].clone();
        fence.extend_from_slice(&left.to_be_bytes());
        let parent = frame(&parent, &[fence], 0);
        after[root..root + 4096].copy_from_slice(&parent);
        after.extend_from_slice(&leaf(case, left, 0, &keys[200..]));
        after.extend_from_slice(&leaf(case, 0, right, &keys[..200]));
    }

    let mut changes = BTreeMap::<usize, (u8, u8, u8)>::new();
    for &(offset, mask, original, set) in case.claims {
        assert_eq!(before[offset], original);
        assert_eq!(original & mask != 0, !set);
        let entry = changes.entry(offset).or_insert((original, 0, 0));
        assert_eq!(entry.0, original);
        if set {
            entry.1 |= mask;
        } else {
            entry.2 |= mask;
        }
    }
    for (offset, (old, sets, clears)) in changes {
        assert_eq!(sets & clears, 0);
        after[offset] = (old | sets) & !clears;
    }
    assert_eq!(after.len(), before.len() + 16384);
    after
}

fn rows(case: &Case, inserted: bool) -> Vec<Vec<Value>> {
    let mut result: Vec<_> = (1..=400)
        .map(|id| {
            vec![
                Value::Int32(id),
                if case.money {
                    Value::Currency(Currency::from_scaled(0))
                } else {
                    Value::Int64(0)
                },
                Value::Int32(id * 17),
            ]
        })
        .collect();
    if inserted {
        result.push(vec![
            Value::Int32(401),
            if case.money {
                Value::Currency(Currency::from_scaled(i64::MAX))
            } else {
                Value::Int64(i64::MAX)
            },
            Value::Int32(6817),
        ]);
    }
    result
}

fn observe(case: &Case, raw: &[u8], inserted: bool) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let catalog = crate::inspect_table_schema(
        &mut image.reader(),
        SCOPE,
        "MSysObjects",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(catalog.columns().len(), 17);
    let source = super::capture(&image, case.table, &mut budget(), &|| false).unwrap();
    let expected = rows(case, inserted);
    assert_eq!(source.table.rows(), expected);
    assert_eq!(source.table.schema().definition_page(), case.definition);
    assert_eq!(source.records.len(), expected.len());
    for (ordinal, record) in source.records.iter().enumerate() {
        assert_eq!(record.reference, reference(case, ordinal));
        assert_eq!(record.data.len(), 19);
    }
    let checked = crate::table::indexes::check_table_retained(
        &mut image.reader(),
        SCOPE,
        case.table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), expected.len());
    assert_eq!(checked.indexes().len(), 4);
    for (name, physical) in [
        ("PrimaryID", 0),
        ("Ascending", 1),
        ("Descending", 2),
        ("Alias", 1),
    ] {
        let selected = checked
            .indexes()
            .iter()
            .find(|item| item.index().name() == name)
            .unwrap();
        assert_eq!(selected.status(), IndexCheckStatus::Checked);
        assert_eq!(selected.index().root_page(), case.roots[physical]);
        assert_eq!(selected.index().data_number(), physical as u32);
        let counter = inspect_table_index_counter(
            &mut image.reader(),
            SCOPE,
            case.table,
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            counter.value(),
            if physical == 0 {
                expected.len() as u32
            } else if inserted {
                2
            } else {
                1
            }
        );
        let read = read_table_index(
            &mut image.reader(),
            SCOPE,
            case.table,
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut ordered = expected.clone();
        if inserted && physical == 2 {
            ordered.rotate_right(1);
        }
        assert_eq!(read.rows(), ordered);
        let mut input = image.reader();
        let mut work = budget();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget: &mut work,
            cancelled: &|| false,
            reads: 0,
        };
        let owned = allocation::pages(&mut reader, selected.index().usage_map()).unwrap();
        let mut expected_owned = vec![case.roots[physical]];
        if inserted && physical != 0 {
            let right = case.eof + ((physical - 1) * 2) as u32;
            expected_owned.extend([right, right + 1]);
            expected_owned.sort();
        }
        assert_eq!(owned, expected_owned);
    }
    let constraints = check_table_constraints(
        &mut image.reader(),
        SCOPE,
        case.table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), expected.len());
    assert!(constraints.relationships().is_empty());
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires six independently constructed wider root promotion source files"]
fn wider_root_promotions_source_eight_contexts_replay_complete_images_before_mutation() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_WIDE_ROOT_PROMOTION_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let cases: Vec<Case> = include!("wide_root_promotion_sources/pins.rs");
    // Pin every input before any Source observation; repeated BEFOREs bind both tables.
    let verified: Vec<_> = cases
        .into_iter()
        .map(|case| {
            let before = fs::read(directory.join(case.before)).unwrap();
            let after = fs::read(directory.join(case.after)).unwrap();
            assert_eq!(hash(&before), case.before_hash);
            assert_eq!(hash(&after), case.after_hash);
            (case, before, after)
        })
        .collect();
    assert_eq!(verified.len(), 4);

    for (case, before, after) in verified {
        assert_eq!(replay(&case, &before), after);
        observe(&case, &before, false);
        observe(&case, &after, true);
    }
}
