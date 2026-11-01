//! Four pre-existing source/model images; reads and fresh Source only, no planner.
use crate::{
    IndexCheckStatus, NativeImage, check_table_constraints, inspect_table_index_counter,
    inspect_table_properties, inspect_table_schema, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(219);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn u16_at(raw: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(raw[at..at + 2].try_into().unwrap()))
}

fn u32_at(raw: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(raw[at..at + 4].try_into().unwrap())
}

fn frame(raw: &[u8], page: usize, slot: usize) -> (usize, usize) {
    let at = page * 4096;
    assert_eq!(&raw[at..at + 2], &[1, 1]);
    assert!(slot < u16_at(raw, at + 12));
    let word = u16_at(raw, at + 14 + 2 * slot);
    assert_eq!(word & !8191, 0);
    let start = word;
    let end = if slot == 0 {
        4096
    } else {
        u16_at(raw, at + 12 + 2 * slot) & 8191
    };
    assert!(14 + 2 * u16_at(raw, at + 12) <= start && start < end && end <= 4096);
    (at + start, at + end)
}

fn record(id: i32, present: bool) -> Vec<u8> {
    let mut raw = 2_u16.to_le_bytes().to_vec();
    raw.extend_from_slice(&id.to_le_bytes());
    raw.extend_from_slice(&[6, 0, 6, 0, 1, 0, if present { 3 } else { 1 }]);
    raw
}

fn entry(id: i32, reference: u32) -> Vec<u8> {
    let mut raw = vec![127];
    raw.extend_from_slice(&((id as u32) ^ 0x8000_0000).to_be_bytes());
    raw.extend_from_slice(&reference.to_be_bytes());
    raw
}

fn source_rows() -> Vec<Vec<Value>> {
    (-226_i32..=225)
        .filter(|id| *id != 0)
        .enumerate()
        .map(|(ordinal, id)| {
            vec![
                Value::Int32(id),
                if ordinal % 2 == 0 {
                    Value::Null
                } else {
                    Value::Binary(Vec::new())
                },
            ]
        })
        .collect()
}

fn fresh_leaf(td: u32, entries: &[Vec<u8>], previous: u32, next: u32) -> Vec<u8> {
    assert_eq!(entries.len(), 226);
    let mut raw = vec![0; 4096];
    raw[..2].copy_from_slice(&[4, 1]);
    raw[2..4].copy_from_slice(&1582_u16.to_le_bytes());
    raw[4..8].copy_from_slice(&td.to_le_bytes());
    raw[12..16].copy_from_slice(&previous.to_le_bytes());
    raw[16..20].copy_from_slice(&next.to_le_bytes());
    for (position, item) in entries.iter().enumerate() {
        assert_eq!(item.len(), 9);
        let end = 9 * (position + 1);
        raw[480 + end - 9..480 + end].copy_from_slice(item);
        raw[27 + end / 8] |= 1 << (end % 8);
    }
    raw
}

fn literal(original: &[u8], td: u32, root: u32, first: u32, last: u32) -> Vec<u8> {
    let mut keys = Vec::new();
    for (ordinal, id) in (-226_i32..=225).filter(|id| *id != 0).enumerate() {
        let (page, slot) = if ordinal < 255 {
            (first, ordinal)
        } else {
            (last, ordinal - 255)
        };
        let (start, end) = frame(original, page as usize, slot);
        assert_eq!(&original[start..end], record(id, ordinal % 2 != 0));
        keys.push(entry(id, (page << 8) | slot as u32));
    }
    let root_at = root as usize * 4096;
    assert_eq!(&original[root_at..root_at + 2], &[4, 1]);
    assert_eq!(u16_at(original, root_at + 24), 1);
    assert_eq!(u16_at(original, root_at + 2), 7);
    let mut active = Vec::new();
    let mut bitmap = [0_u8; 453];
    for (position, key) in keys.iter().enumerate() {
        active.extend_from_slice(if position == 0 { key } else { &key[1..] });
        let end = active.len();
        bitmap[end / 8] |= 1 << (end % 8);
    }
    assert_eq!(active.len(), 3609);
    assert_eq!(&original[root_at + 27..root_at + 480], &bitmap);
    assert_eq!(&original[root_at + 480..root_at + 4089], active);

    let td_at = td as usize * 4096;
    let data_at = last as usize * 4096;
    assert_eq!(u16_at(original, data_at + 12), 196);
    assert_eq!(u16_at(original, data_at + 2), 1142);
    let frontier = u16_at(original, data_at + 14 + 2 * 195);
    let start = frontier - 13;
    let mut after = original.to_vec();
    after[data_at + start..data_at + frontier].copy_from_slice(&record(226, true));
    after[data_at + 14 + 2 * 196..data_at + 16 + 2 * 196]
        .copy_from_slice(&(start as u16).to_le_bytes());
    after[data_at + 12..data_at + 14].copy_from_slice(&197_u16.to_le_bytes());
    after[data_at + 2..data_at + 4].copy_from_slice(&1127_u16.to_le_bytes());
    keys.push(entry(226, (last << 8) | 196));
    assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
    let right = (original.len() / 4096) as u32;
    let left = right + 1;
    after.extend_from_slice(&fresh_leaf(td, &keys[226..], left, 0));
    after.extend_from_slice(&fresh_leaf(td, &keys[..226], 0, right));

    after[root_at] = 3;
    after[root_at + 2..root_at + 4].copy_from_slice(&3603_u16.to_le_bytes());
    after[root_at + 12..root_at + 20].fill(0);
    after[root_at + 20..root_at + 24].copy_from_slice(&right.to_le_bytes());
    after[root_at + 24..root_at + 26].fill(0);
    after[root_at + 26] = 1;
    after[root_at + 27..root_at + 480].fill(0);
    after[root_at + 28] = 32;
    after[root_at + 480..root_at + 489].copy_from_slice(&keys[225]);
    after[root_at + 489..root_at + 493].copy_from_slice(&left.to_be_bytes());
    assert_eq!(
        &after[root_at + 493..root_at + 4096],
        &original[root_at + 493..root_at + 4096]
    );
    for at in [td_at + 16, td_at + 67] {
        assert_eq!(u32_at(original, at), 451);
        after[at..at + 4].copy_from_slice(&452_u32.to_le_bytes());
    }

    // Frozen sources have explicit GLOBAL future FREE=1, INDEX-owned=0.
    // This does not establish a policy for unrepresented or represented-zero EOF.
    let owned_ref = u32_at(original, td_at + 55);
    let (global, global_end) = frame(original, 1, 0);
    let (owned, owned_end) = frame(original, (owned_ref >> 8) as usize, 2);
    assert_eq!((global_end - global, owned_end - owned), (69, 69));
    assert_eq!(original[global], 0);
    assert_eq!(u32_at(original, global + 1), 0);
    assert_eq!(original[owned], 0);
    let base = u32_at(original, owned + 1);
    assert_eq!(base, root);
    for number in [right, left] {
        let cell = global + 5 + number as usize / 8;
        let mask = 1 << (number % 8);
        assert_ne!(original[cell] & mask, 0);
        after[cell] &= !mask;
        let bit = (number - base) as usize;
        let cell = owned + 5 + bit / 8;
        let mask = 1 << (bit % 8);
        assert_eq!(original[cell] & mask, 0);
        after[cell] |= mask;
    }
    after
}

fn inspect(raw: &[u8], td: u32, expected: &[Vec<Value>]) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let catalog = inspect_table_schema(
        &mut image.reader(),
        SCOPE,
        "MSysObjects",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(catalog.columns().len(), 17);
    let snapshot = read_table(
        &mut image.reader(),
        SCOPE,
        "BinaryStorage",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(snapshot.rows(), expected);
    assert_eq!(snapshot.schema().definition_page(), td);
    let fields = snapshot.schema().columns();
    assert_eq!(fields.len(), 2);
    assert_eq!(
        (fields[0].name(), fields[0].native_type(), fields[0].flags()),
        ("ID", 4, 3)
    );
    assert_eq!(
        (
            fields[1].name(),
            fields[1].native_type(),
            fields[1].storage_length()
        ),
        ("Bytes", 9, 255)
    );
    let properties = inspect_table_properties(
        &mut image.reader(),
        SCOPE,
        "BinaryStorage",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(!properties.raw_bytes().is_empty());
    let group = properties
        .groups()
        .iter()
        .find(|group| group.name() == "ID")
        .unwrap();
    assert!(group.get("Required").unwrap().boolean().unwrap());
    let constraints = check_table_constraints(
        &mut image.reader(),
        SCOPE,
        "BinaryStorage",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), expected.len());
    assert!(constraints.relationships().is_empty());
    let checked = crate::table::indexes::check_table_retained(
        &mut image.reader(),
        SCOPE,
        "BinaryStorage",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), expected.len());
    assert_eq!(checked.indexes().len(), 1);
    assert_eq!(checked.indexes()[0].status(), IndexCheckStatus::Checked);
    let index = checked.indexes()[0].index();
    assert_eq!(index.name(), "PrimaryID");
    assert_eq!(index.root_page(), td + 2);
    assert_eq!(index.usage_map(), ((td + 1) << 8) | 2);
    let flags = index.constraints().unwrap();
    assert!(flags.primary_key() && flags.unique() && flags.required());
    assert!(!flags.ignore_nulls());
    let rows = read_table_index(
        &mut image.reader(),
        SCOPE,
        "BinaryStorage",
        "PrimaryID",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rows.rows(), expected);
    let counter = inspect_table_index_counter(
        &mut image.reader(),
        SCOPE,
        "BinaryStorage",
        "PrimaryID",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(counter.value() as usize, expected.len());

    // Exact current normal action Source and NoRequired conjunction. A first
    // refusal is a diagnostic, not permission to strip or retune the source.
    let source =
        super::super::normal_actions::capture(&image, "BinaryStorage", &mut budget(), &|| false)
            .expect("current full Source/NoRequired root-split source qualification");
    assert_eq!(source.records.len(), expected.len());
    assert_eq!(source.table.rows(), expected);
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires four frozen original/model root-split images; no producers"]
fn existing_genuine_root_split_originals_and_models_have_complete_current_source_coverage() {
    let repository = PathBuf::from(std::env::var_os("TABLECRAFT_ROOT_SPLIT_REPOSITORY").unwrap());
    assert!(repository.is_absolute());
    for (before_path, before_hash, after_path, after_hash, td, root, first, last) in [
        (
            ".cache/ace16-insert-root-split-reference-first-20261027/V2016-fitted-source.accdb",
            "3226fb48824cae7bf9de03da58df2946c58acd239d33c6604c78a0a5473e6932",
            ".cache/ace16-insert-root-split-independent-next-20261027/expected/V2016-fitted.expected.bin",
            "d8ade2dbaa5ce075f95b08e8c801a07864221cb9772eab8de95ed44dee418d60",
            89_u32,
            91_u32,
            92_u32,
            94_u32,
        ),
        (
            ".cache/ace16-insert-root-split-reference-first-20261027/V2019-fitted-source.accdb",
            "00badd8da31daedd3b1ce3637d334e251158fd2fbd7e02f264860c771e4aa489",
            ".cache/ace16-insert-root-split-independent-next-20261027/expected/V2019-fitted.expected.bin",
            "9117623259fa78d90fe155aec29debbf004d10d77fe5ff8b146d3b950d9e230c",
            106_u32,
            108_u32,
            110_u32,
            112_u32,
        ),
    ] {
        let before = fs::read(repository.join(before_path)).unwrap();
        let after = fs::read(repository.join(after_path)).unwrap();
        assert_eq!(hash(&before), before_hash);
        assert_eq!(hash(&after), after_hash);
        assert_eq!(literal(&before, td, root, first, last), after);
        let original = source_rows();
        inspect(&before, td, &original);
        let mut rows = original;
        rows.push(vec![Value::Int32(226), Value::Binary(Vec::new())]);
        inspect(&after, td, &rows);
    }
}
