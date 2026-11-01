//! Independent original-only EOF allocation images; no native writer API.
use crate::format::{allocation, reader::Reader};
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

fn read_view(bytes: &[u8], definition: u32, maps: u32, marker: Option<&str>) {
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
        [vec![
            Value::Int32(23),
            marker.map_or(Value::Null, |value| Value::Text(value.into())),
            Value::Binary(vec![199, 0, 255])
        ]]
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
    assert_eq!(lval.rows(), 1);
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
        (0, 1, 0, 0)
    );
    let coverage = image
        .inspect_table_macro_coverage(table.schema().object_id(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(coverage.body().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.module().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.table_extra().presence(), PayloadPresence::Absent);
    assert!(coverage.children().is_empty());
    let preserved = crate::table::mutation::normal_ordinary::capture(
        &image,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(preserved.table.rows(), table.rows());
    assert_eq!(image.as_bytes(), bytes);
}

fn memberships(raw: &[u8], maps: u32, old: u32, new: u32, after: bool) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let mut bytes = image.reader();
    let mut work = budget();
    let mut reader = Reader {
        source: &mut bytes,
        header: image.catalog().header(),
        budget: &mut work,
        cancelled: &|| false,
        reads: 0,
    };
    let expected = if after { vec![old, new] } else { vec![old] };
    assert_eq!(allocation::pages(&mut reader, maps << 8).unwrap(), expected);
    // Original saturated root hint is retained; only the EOF bit is added.
    assert_eq!(
        allocation::pages(&mut reader, (maps << 8) | 1).unwrap(),
        expected
    );
    let global = allocation::RepresentedMap::read(
        &mut reader,
        allocation::GLOBAL_FREE_REFERENCE,
        allocation::MapRole::GlobalFree,
    )
    .unwrap();
    global.require_allocated(&mut reader, old).unwrap();
    if after {
        global.require_allocated(&mut reader, new).unwrap();
    }
}

#[test]
#[ignore = "requires explicit genuine originals and authored alternate-owned source directory"]
fn genuine_eof_allocation_sources_and_literal_after_read_before_any_writer() {
    let originals =
        PathBuf::from(std::env::var_os("TABLECRAFT_PROPERTY_LVAL_ORIGINALS_DIR").unwrap());
    let sources =
        PathBuf::from(std::env::var_os("TABLECRAFT_EOF_ALLOCATION_EXPECTED_DIR").unwrap());
    assert!(originals.is_absolute() && sources.is_absolute());
    let mut views = 0;
    for (header, digest, definition, maps, bitmap, old, new) in [
        (
            5_u8,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            107_u32,
            108_u32,
            111_usize,
            110_u32,
            112_u32,
        ),
        (
            6_u8,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            125_u32,
            126_u32,
            128_usize,
            127_u32,
            129_u32,
        ),
    ] {
        let path = originals.join(format!("h{header}.accdb"));
        let original = fs::read(&path).unwrap();
        assert_eq!(hash(&original), digest);
        assert_eq!(original.len(), new as usize * 4096);
        let base = old as usize * 4096;
        let mut before = original.clone();
        let mut empty = vec![0; 4096];
        empty[..4].copy_from_slice(&[1, 1, 242, 15]);
        empty[4..8].copy_from_slice(&definition.to_le_bytes());
        before[base + 2..base + 4].copy_from_slice(&[178, 13]);
        before[base + 12..base + 14].copy_from_slice(&[255, 0]);
        for slot in 1..255 {
            let at = base + 14 + slot * 2;
            before[at..at + 2].copy_from_slice(&[190, 143]);
        }
        let owned = bitmap * 4096 + 4 + new as usize / 8;
        let candidate = maps as usize * 4096 + 3958 + 5 + new as usize / 8;
        let global = 4096 + 4027 + 5 + new as usize / 8;
        let bit = 1 << (new % 8);
        assert_eq!(original[owned] & bit, 0);
        assert_eq!(original[candidate] & bit, 0);
        assert_eq!(original[global] & bit, bit);
        let source =
            fs::read(sources.join(format!("h{header}-eof-allocation-before.accdb"))).unwrap();
        assert_eq!(source, before);
        read_view(&source, definition, maps, Some("unchanged sentinel"));
        memberships(&source, maps, old, new, false);
        let row = &original[base + 4030..base + 4096];
        let mut terminal = row[..6].to_vec();
        for _ in 0..19 {
            terminal.extend_from_slice(&[97, 0]);
        }
        terminal.extend_from_slice(&row[42..57]);
        terminal.extend_from_slice(&[59, 0, 44, 0, 6, 0, 2, 0, 7]);
        assert_eq!(terminal.len(), 68);
        let next = new as usize * 4096;
        let mut literal = source.clone();
        literal.extend_from_slice(&empty);
        literal[owned] |= bit;
        literal[candidate] |= bit;
        literal[global] &= !bit;
        literal[next + 4028..next + 4096].copy_from_slice(&terminal);
        literal[next + 2..next + 4].copy_from_slice(&[172, 15]);
        literal[next + 12..next + 16].copy_from_slice(&[1, 0, 188, 143]);
        literal[base + 14..base + 16].copy_from_slice(&[190, 79]);
        literal[base + 4030..base + 4096].fill(0);
        literal[base + 4030..base + 4034].copy_from_slice(&(new << 8).to_le_bytes());
        let after =
            fs::read(sources.join(format!("h{header}-eof-allocation-growth.accdb"))).unwrap();
        assert_eq!(after, literal);
        assert_eq!(after.len(), source.len() + 4096);
        read_view(&after, definition, maps, Some("aaaaaaaaaaaaaaaaaaa"));
        memberships(&after, maps, old, new, true);
        assert_eq!(fs::read(path).unwrap(), original);
        views += 2;
    }
    assert_eq!(views, 4);
}
