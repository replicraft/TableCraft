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
    if inserted {
        for id in 24..=623 {
            expected.push(vec![Value::Int32(id), Value::Null, Value::Null]);
        }
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
    assert_eq!(lval.rows(), if inserted { 601 } else { 1 });
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
        (if inserted { 600 } else { 0 }, 1, 0, 0)
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
#[ignore = "requires independently modeled two-page INSERT source views and full SDK qualification"]
fn multipage_insert_sources_preserve_ole_and_coalesce_six_bits_in_three_map_bytes() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_NORMAL_MULTIPAGE_INSERT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (header, length, definition, maps, data, bitmap, eof, before_hash, after_hash) in [
        (
            5,
            458752,
            107,
            108,
            110,
            111,
            112,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            "1f89ca37898bb5374331c673d633b70d06c9f2c75f3b4458a7647f743fb552db",
        ),
        (
            6,
            528384,
            125,
            126,
            127,
            128,
            129,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            "d9ea6bfbea2b0d6dd0c95a4cfd543cda2ad4a2dd57523f5ff7c7c34847e544d0",
        ),
    ] {
        let before =
            fs::read(directory.join(format!("h{header}-insert-multipage-before.accdb"))).unwrap();
        let after =
            fs::read(directory.join(format!("h{header}-insert-multipage-after.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(after.len(), length + 8192);
        assert_eq!(hash(&before), before_hash);
        assert_eq!(hash(&after), after_hash);
        cases.push((header, before, after, definition, maps, data, bitmap, eof));
    }
    for (header, before, after, definition, maps, data, bitmap, eof) in cases {
        read_view(&before, definition, maps, false);
        read_view(&after, definition, maps, true);
        let td = definition as usize * 4096;
        let root = data as usize * 4096;
        assert_eq!(&before[td + 16..td + 20], &1_u32.to_le_bytes());
        assert_eq!(&after[td + 16..td + 20], &601_u32.to_le_bytes());
        assert_eq!(&after[root + 2..root + 4], &2_u16.to_le_bytes());
        assert_eq!(&after[root + 12..root + 14], &237_u16.to_le_bytes());
        assert_eq!(
            &after[root + 4030..root + 4096],
            &before[root + 4030..root + 4096]
        );
        for (number, count, free, frontier) in [(eof, 240, 2, 496), (eof + 1, 124, 1974, 2236)] {
            let page = number as usize * 4096;
            assert_eq!(&after[page..page + 2], &[1, 1]);
            assert_eq!(&after[page + 4..page + 8], &definition.to_le_bytes());
            assert_eq!(&after[page + 2..page + 4], &(free as u16).to_le_bytes());
            assert_eq!(&after[page + 12..page + 14], &(count as u16).to_le_bytes());
            assert_eq!(
                &after[page + 12 + 2 * count..page + 14 + 2 * count],
                &(frontier as u16).to_le_bytes()
            );
        }
        for position in 0..600 {
            let (page, slot, end) = if position < 236 {
                (root, position + 1, 4030 - 15 * position)
            } else if position < 476 {
                (
                    eof as usize * 4096,
                    position - 236,
                    4096 - 15 * (position - 236),
                )
            } else {
                (
                    (eof + 1) as usize * 4096,
                    position - 476,
                    4096 - 15 * (position - 476),
                )
            };
            let start = end - 15;
            assert_eq!(
                &after[page + 14 + 2 * slot..page + 16 + 2 * slot],
                &(start as u16).to_le_bytes()
            );
            let mut record = vec![3, 0];
            record.extend_from_slice(&(24_i32 + position as i32).to_le_bytes());
            record.extend_from_slice(&[6, 0, 6, 0, 6, 0, 2, 0, 1]);
            assert_eq!(&after[page + start..page + end], record);
        }
        let cells = [
            4096 + 4032 + eof as usize / 8,
            bitmap as usize * 4096 + 4 + eof as usize / 8,
            maps as usize * 4096 + 3963 + eof as usize / 8,
        ];
        let union = if header == 5 { 3 } else { 6 };
        for (position, at) in cells.into_iter().enumerate() {
            assert_eq!(before[at] ^ after[at], union);
            assert_eq!(before[at] & union, if position == 0 { union } else { 0 });
            assert_eq!(after[at] & union, if position == 0 { 0 } else { union });
        }
        let allowed = [
            td + 16..td + 20,
            root + 2..root + 4,
            root + 12..root + 488,
            root + 490..root + 4030,
            cells[0]..cells[0] + 1,
            cells[1]..cells[1] + 1,
            cells[2]..cells[2] + 1,
        ];
        for at in 0..before.len() {
            if !allowed.iter().any(|span| span.contains(&at)) {
                assert_eq!(
                    before[at], after[at],
                    "outside multi-page INSERT spans at {at}"
                );
            }
        }
    }
}
