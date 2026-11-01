//! Original-only literal INSERT source reads; no insert planner or journal calls.
use crate::{
    LongValueCheckStatus, NativeImage, PayloadPresence, PropertyGroupKind, check_table_long_values,
    inspect_table_indexes, inspect_table_properties, read_table,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(155);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn read_view(bytes: &[u8], definition: u32, maps: u32, inserted: bool) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let table = read_table(
        &mut image.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.schema().definition_page(), definition);
    assert_eq!(table.columns().len(), 3);
    let mut expected = vec![vec![
        Value::Int32(23),
        Value::Text("unchanged sentinel".into()),
        Value::Binary(vec![199, 0, 255]),
    ]];
    for id in 24..=if inserted { 260 } else { 259 } {
        expected.push(vec![Value::Int32(id), Value::Null, Value::Null]);
    }
    assert_eq!(table.rows(), expected);
    let fields = table.schema().columns();
    assert_eq!(
        (
            fields[0].name(),
            fields[0].native_type(),
            fields[0].flags(),
            fields[0].storage_length()
        ),
        ("ID", 4, 3, 4)
    );
    assert_eq!(
        (
            fields[1].name(),
            fields[1].native_type(),
            fields[1].flags(),
            fields[1].extended_flags(),
            fields[1].storage_length()
        ),
        ("Marker", 10, 2, 0, 80)
    );
    assert_eq!(
        (
            fields[2].name(),
            fields[2].native_type(),
            fields[2].flags(),
            fields[2].extended_flags(),
            fields[2].storage_length()
        ),
        ("Bytes", 11, 2, 0, 0)
    );
    let indexes = inspect_table_indexes(
        &mut image.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(indexes.indexes().is_empty());
    let properties = inspect_table_properties(
        &mut image.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let table_groups = properties
        .groups()
        .iter()
        .filter(|group| group.kind() == PropertyGroupKind::Table)
        .collect::<Vec<_>>();
    assert_eq!(table_groups.len(), 1);
    assert_eq!(table_groups[0].properties().len(), 1);
    let property = table_groups[0].get("PublishToWeb").unwrap();
    assert_eq!(
        (
            property.native_type(),
            property.flags(),
            property.raw_value()
        ),
        (2, 0, &[0][..])
    );
    let lval = check_table_long_values(
        &mut image.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(lval.rows(), if inserted { 238 } else { 237 });
    assert_eq!(lval.columns().len(), 1);
    let bytes_column = &lval.columns()[0];
    assert_eq!(bytes_column.column_number(), 2);
    assert_eq!(bytes_column.status(), LongValueCheckStatus::Checked);
    let references = bytes_column.maps().unwrap();
    assert_eq!(
        (references.owned(), references.candidate()),
        ((maps << 8) | 2, (maps << 8) | 3)
    );
    assert_eq!(
        (
            bytes_column.owned_pages(),
            bytes_column.candidate_pages(),
            bytes_column.owned_rows(),
            bytes_column.distinct_chunks()
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(
        (
            bytes_column.values().nulls(),
            bytes_column.values().inline(),
            bytes_column.values().single(),
            bytes_column.values().chained()
        ),
        (if inserted { 237 } else { 236 }, 1, 0, 0)
    );
    let coverage = image
        .inspect_table_macro_coverage(table.schema().object_id(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(coverage.body().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.module().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.table_extra().presence(), PayloadPresence::Absent);
    assert!(coverage.children().is_empty());
    image
        .check_native_ordinary_fitted_table("Sentinel", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
#[ignore = "requires four pure-source near-capacity/single EOF INSERT views and SDK4 observations"]
fn single_eof_insert_sources_qualify_healthy_near_capacity_and_one_new_visible_row() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_NORMAL_SINGLE_INSERT_EOF_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (header, bytes, definition, maps, data, bitmap, eof, before_hash, after_hash) in [
        (
            5,
            458752,
            107,
            108,
            110,
            111,
            112,
            "9934f7d78854336521bfed4375712fd8ffad283855ae14ee8b3d833c681ff3fa",
            "91bfe1d2332e2173f88810fea2053ee1ce531b3f7019692eedc41339b2b12b2e",
        ),
        (
            6,
            528384,
            125,
            126,
            127,
            128,
            129,
            "3737bed0bc09ac9aa2f4fb780d5bbf771dbd5134c8f4170e12b82ce84b563272",
            "3c2e562e8cfc91bf299e9fce6394bddfd9762e5ce25aa05a9d97171072611a4d",
        ),
    ] {
        let before =
            fs::read(directory.join(format!("h{header}-single-insert-eof-before.accdb"))).unwrap();
        let after =
            fs::read(directory.join(format!("h{header}-single-insert-eof-after.accdb"))).unwrap();
        assert_eq!(before.len(), bytes);
        assert_eq!(after.len(), bytes + 4096);
        assert_eq!(hash(&before), before_hash);
        assert_eq!(hash(&after), after_hash);
        cases.push((before, after, definition, maps, data, bitmap, eof));
    }
    for (before, after, definition, maps, data, bitmap, eof) in cases {
        read_view(&before, definition, maps, false);
        read_view(&after, definition, maps, true);
        let td = definition as usize * 4096;
        let root = data as usize * 4096;
        let target = eof as usize * 4096;
        assert_eq!(&before[td + 16..td + 20], &237_u32.to_le_bytes());
        assert_eq!(&after[td + 16..td + 20], &238_u32.to_le_bytes());
        assert_eq!(&before[root + 2..root + 4], &2_u16.to_le_bytes());
        assert_eq!(&before[root + 12..root + 14], &237_u16.to_le_bytes());
        assert_eq!(&before[root..root + 4096], &after[root..root + 4096]);
        let mut expected_page = vec![0; 4096];
        expected_page[..2].copy_from_slice(&[1, 1]);
        expected_page[2..4].copy_from_slice(&4065_u16.to_le_bytes());
        expected_page[4..8].copy_from_slice(&definition.to_le_bytes());
        expected_page[12..16].copy_from_slice(&[1, 0, 241, 15]);
        expected_page[4081..].copy_from_slice(&[3, 0, 4, 1, 0, 0, 6, 0, 6, 0, 6, 0, 2, 0, 1]);
        assert_eq!(&after[target..], expected_page);
        let cells = [
            4096 + 4027 + 5 + eof as usize / 8,
            bitmap as usize * 4096 + 4 + eof as usize / 8,
            maps as usize * 4096 + 3958 + 5 + eof as usize / 8,
        ];
        let mask = 1 << (eof % 8);
        for (position, at) in cells.into_iter().enumerate() {
            assert_eq!(before[at] ^ after[at], mask);
            assert_eq!(before[at] & mask != 0, position == 0);
            assert_eq!(after[at] & mask != 0, position != 0);
        }
        for at in 0..before.len() {
            if !(td + 16..td + 20).contains(&at) && !cells.contains(&at) {
                assert_eq!(
                    before[at], after[at],
                    "outside independent single INSERT spans at {at}"
                );
            }
        }
    }
}
