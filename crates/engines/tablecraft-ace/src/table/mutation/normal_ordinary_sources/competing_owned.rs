//! Independent literal same-owned-page forwarding source reads, no writer API.
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

fn empty(definition: u32) -> Vec<u8> {
    let mut raw = vec![0; 4096];
    raw[..4].copy_from_slice(&[1, 1, 242, 15]);
    raw[4..8].copy_from_slice(&definition.to_le_bytes());
    raw
}

#[derive(Clone, Copy)]
struct Shape {
    definition: u32,
    maps: usize,
    bitmap: usize,
    root: usize,
    first: usize,
    second: usize,
}

fn source(parent: &[u8], shape: Shape, full: bool) -> Vec<u8> {
    let Shape {
        definition,
        maps,
        bitmap,
        root,
        first,
        second,
    } = shape;
    let rb = root * 4096;
    let fb = first * 4096;
    let sb = second * 4096;
    assert_eq!(parent.len(), sb);
    let mut result = parent.to_vec();
    result.extend_from_slice(&empty(definition));
    let mut row = parent[rb + 4030..rb + 4096].to_vec();
    assert_eq!(&row[..6], &[3, 0, 23, 0, 0, 0]);
    row[2..6].copy_from_slice(&24_i32.to_le_bytes());
    result[rb + 3964..rb + 4030].copy_from_slice(&row);
    result[rb + 16..rb + 18].copy_from_slice(&[124, 15]);
    for slot in 2..255 {
        let at = rb + 14 + slot * 2;
        result[at..at + 2].copy_from_slice(&[124, 143]);
    }
    result[rb + 2..rb + 4].copy_from_slice(&[112, 13]);
    result[definition as usize * 4096 + 16..definition as usize * 4096 + 20]
        .copy_from_slice(&2_u32.to_le_bytes());
    result[fb + 2..fb + 4].copy_from_slice(&[246, 13]);
    result[fb + 12..fb + 14].copy_from_slice(&[254, 0]);
    for slot in 0..254 {
        let at = fb + 14 + slot * 2;
        result[at..at + 2].copy_from_slice(&[0, 144]);
    }
    result[bitmap * 4096 + 4 + second / 8] |= 1 << (second % 8);
    result[maps * 4096 + 3958 + 5 + second / 8] |= 1 << (second % 8);
    result[4096 + 4027 + 5 + second / 8] &= !(1 << (second % 8));
    if full {
        result[sb + 2..sb + 4].copy_from_slice(&[244, 13]);
        result[sb + 12..sb + 14].copy_from_slice(&[255, 0]);
        for slot in 0..255 {
            let at = sb + 14 + slot * 2;
            result[at..at + 2].copy_from_slice(&[0, 144]);
        }
    }
    result
}

fn literal(before: &[u8], root: usize, first: usize, second: usize) -> Vec<u8> {
    let mut after = before.to_vec();
    for (ordinal, start, page, slot, letter, free) in [
        (0, 4030, first, 254, b'a', 3504_u16),
        (1, 3964, second, 0, b'c', 4012_u16),
    ] {
        let rb = root * 4096;
        let tb = page * 4096;
        let mut terminal = before[rb + start..rb + start + 6].to_vec();
        for _ in 0..19 {
            terminal.extend_from_slice(&[letter, 0]);
        }
        terminal.extend_from_slice(&before[rb + start + 42..rb + start + 57]);
        terminal.extend_from_slice(&[59, 0, 44, 0, 6, 0, 2, 0, 7]);
        assert_eq!(terminal.len(), 68);
        after[tb + 4028..tb + 4096].copy_from_slice(&terminal);
        after[tb + 2..tb + 4].copy_from_slice(&free.to_le_bytes());
        after[tb + 12..tb + 14].copy_from_slice(&((slot + 1) as u16).to_le_bytes());
        after[tb + 14 + slot * 2..tb + 16 + slot * 2].copy_from_slice(&[188, 143]);
        after[rb + start..rb + start + 66].fill(0);
        after[rb + start..rb + start + 4]
            .copy_from_slice(&(((page as u32) << 8) | slot as u32).to_le_bytes());
        after[rb + 14 + ordinal * 2..rb + 16 + ordinal * 2]
            .copy_from_slice(&((start as u16) | 0x4000).to_le_bytes());
    }
    after
}

#[test]
#[ignore = "requires independently authored two-root SOURCE/AFTER files"]
fn competing_owned_reservations_sources_read_before_any_writer() {
    let parent =
        PathBuf::from(std::env::var_os("TABLECRAFT_ALTERNATE_OWNED_EXPECTED_DIR").unwrap());
    let expected =
        PathBuf::from(std::env::var_os("TABLECRAFT_MULTIROW_OWNED_EXPECTED_DIR").unwrap());
    assert!(parent.is_absolute() && expected.is_absolute());
    let mut views = 0;
    for (header, digest, definition, maps, bitmap, root, first, second) in [
        (
            5_u8,
            "a8954a42742b6414726055970c628c88436059d7e14f919d939cae95d35c4cbd",
            107_u32,
            108,
            111,
            110,
            112,
            113,
        ),
        (
            6_u8,
            "a0b3bf39db8322a21b90cde1c72d168f1ece059c268fae6212eb93c4692b0a21",
            125_u32,
            126,
            128,
            127,
            129,
            130,
        ),
    ] {
        let shape = Shape {
            definition,
            maps,
            bitmap,
            root,
            first,
            second,
        };
        let path = parent.join(format!("h{header}-alternate-owned-before.accdb"));
        let raw = fs::read(&path).unwrap();
        assert_eq!(hash(&raw), digest);
        let before =
            fs::read(expected.join(format!("h{header}-multirow-owned-before.accdb"))).unwrap();
        assert_eq!(before, source(&raw, shape, false));
        read_view(
            &before,
            definition,
            maps as u32,
            "unchanged sentinel",
            "unchanged sentinel",
        );
        let after =
            fs::read(expected.join(format!("h{header}-multirow-owned-growth.accdb"))).unwrap();
        assert_eq!(after, literal(&before, root, first, second));
        read_view(
            &after,
            definition,
            maps as u32,
            "aaaaaaaaaaaaaaaaaaa",
            "ccccccccccccccccccc",
        );
        let negative =
            fs::read(expected.join(format!("h{header}-multirow-owned-no-capacity.accdb"))).unwrap();
        assert_eq!(negative, source(&raw, shape, true));
        read_view(
            &negative,
            definition,
            maps as u32,
            "unchanged sentinel",
            "unchanged sentinel",
        );
        assert_eq!(fs::read(path).unwrap(), raw);
        views += 3;
    }
    assert_eq!(views, 6);
}
