//! Independent literal owned-plus-EOF source reads; no planner or writer calls.
use crate::{
    LongValueCheckStatus, NativeImage, PayloadPresence, PropertyGroupKind, check_table_long_values,
    inspect_table_indexes, inspect_table_properties, read_table,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(141);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn read_view(bytes: &[u8], definition: u32, maps: u32, first: &str, second: &str) {
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
    assert_eq!(
        table.rows(),
        [
            vec![
                Value::Int32(23),
                Value::Text(first.into()),
                Value::Binary(vec![199, 0, 255])
            ],
            vec![
                Value::Int32(24),
                Value::Text(second.into()),
                Value::Binary(vec![199, 0, 255])
            ],
        ]
    );
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
    assert_eq!(lval.rows(), 2);
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
        (0, 2, 0, 0)
    );
    let coverage = image
        .inspect_table_macro_coverage(table.schema().object_id(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(coverage.body().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.module().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.table_extra().presence(), PayloadPresence::Absent);
    assert!(coverage.children().is_empty());
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
#[ignore = "requires explicit frozen compound EOF source directory"]
fn independent_compound_owned_and_eof_source_views() {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_COMPOUND_EOF_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    for (header, before_sha, after_sha) in [
        (
            5,
            "2515f4fe4977005335f5aa68a6375ade70b75ffd0dd9f552fa4e62c249a27888",
            "1c555926c1d10f3ee11b6dfa513e38b1dacd0c878602dd7f21b6b9bb5f516cd3",
        ),
        (
            6,
            "cbfaca8d36102e477d3aa7a7b86830169de70b5a64ff6d24b10f2855e81a9bde",
            "2e0050cc028b21b6d5bf940a830bd2fd6e45dfc8a1a4036a7a7fa88e7a563cd2",
        ),
    ] {
        let (definition, maps, bitmap, root, first, second, eof) = match header {
            5 => (107, 108, 111, 110, 112, 113, 114),
            6 => (125, 126, 128, 127, 129, 130, 131),
            _ => unreachable!(),
        };
        let before =
            fs::read(directory.join(format!("h{header}-compound-eof-before.accdb"))).unwrap();
        let after =
            fs::read(directory.join(format!("h{header}-compound-eof-growth.accdb"))).unwrap();
        assert_eq!(hash(&before), before_sha);
        assert_eq!(hash(&after), after_sha);
        assert_eq!(before.len(), eof * 4096);
        assert_eq!(after.len(), before.len() + 4096);
        read_view(
            &before,
            definition,
            maps,
            "unchanged sentinel",
            "unchanged sentinel",
        );
        read_view(
            &after,
            definition,
            maps,
            "aaaaaaaaaaaaaaaaaaa",
            "ccccccccccccccccccc",
        );

        let word = |raw: &[u8], at: usize| u16::from_le_bytes(raw[at..at + 2].try_into().unwrap());
        let pointer =
            |raw: &[u8], at: usize| u32::from_le_bytes(raw[at..at + 4].try_into().unwrap());
        assert_eq!(word(&before, root * 4096 + 12), 255);
        assert_eq!(word(&before, first * 4096 + 12), 254);
        assert_eq!(word(&before, second * 4096 + 12), 255);
        assert_eq!(word(&after, first * 4096 + 12), 255);
        assert_eq!(word(&after, first * 4096 + 2), 3504);
        assert_eq!(word(&after, first * 4096 + 522), 0x8fbc);
        assert_eq!(word(&after, eof * 4096 + 12), 1);
        assert_eq!(word(&after, eof * 4096 + 2), 4012);
        assert_eq!(word(&after, eof * 4096 + 14), 0x8fbc);
        assert_eq!(
            pointer(&after, root * 4096 + 4030),
            ((first as u32) << 8) | 254
        );
        assert_eq!(pointer(&after, root * 4096 + 3964), (eof as u32) << 8);
        assert_eq!(
            &after[second * 4096..(second + 1) * 4096],
            &before[second * 4096..(second + 1) * 4096]
        );
        for (start, old) in [
            (4096 + 4027 + 5, true),
            (bitmap * 4096 + 4, false),
            (maps as usize * 4096 + 3958 + 5, false),
        ] {
            let at = start + eof / 8;
            let mask = 1 << (eof % 8);
            assert_eq!(before[at] & mask != 0, old);
            assert_eq!(after[at] ^ before[at], mask);
        }
    }
}
