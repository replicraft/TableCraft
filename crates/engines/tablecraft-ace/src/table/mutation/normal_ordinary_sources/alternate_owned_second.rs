//! Independent cross-page forwarded second-edit source reads, no writer API.
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
    assert_eq!(image.as_bytes(), bytes);
}

fn memberships(raw: &[u8], maps: u32, old: u32, new: u32) {
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
    assert_eq!(
        allocation::pages(&mut reader, maps << 8).unwrap(),
        [old, new]
    );
    assert_eq!(
        allocation::pages(&mut reader, (maps << 8) | 1).unwrap(),
        [new]
    );
    let global = allocation::RepresentedMap::read(
        &mut reader,
        allocation::GLOBAL_FREE_REFERENCE,
        allocation::MapRole::GlobalFree,
    )
    .unwrap();
    global.require_allocated(&mut reader, old).unwrap();
    global.require_allocated(&mut reader, new).unwrap();
}

#[test]
#[ignore = "requires qualified alternate-owned first and separately authored second images"]
fn alternate_owned_second_sources_read_before_any_writer() {
    let first = PathBuf::from(std::env::var_os("TABLECRAFT_ALTERNATE_OWNED_EXPECTED_DIR").unwrap());
    let second =
        PathBuf::from(std::env::var_os("TABLECRAFT_ALTERNATE_OWNED_SECOND_EXPECTED_DIR").unwrap());
    assert!(first.is_absolute() && second.is_absolute());
    let mut views = 0;
    for (header, digest, definition, maps, old, new) in [
        (
            5_u8,
            "b08e1cb05999cc30970ea97d8ea4b1bbc105ccb6cd05ccda106618914591645e",
            107,
            108,
            110,
            112,
        ),
        (
            6_u8,
            "28ce5406b7fa8b386c4ad6a1060e405c3b7c784d882d0df32f498b1d4b7c97c3",
            125,
            126,
            127,
            129,
        ),
    ] {
        let path = first.join(format!("h{header}-alternate-owned-growth.accdb"));
        let before = fs::read(&path).unwrap();
        assert_eq!(hash(&before), digest);
        read_view(&before, definition, maps, Some("aaaaaaaaaaaaaaaaaaa"));
        memberships(&before, maps, old, new);
        let start = new as usize * 4096 + 4034;
        let end = start + 38;
        for pair in before[start..end].as_chunks::<2>().0 {
            assert_eq!(pair, &[97, 0]);
        }
        let mut literal = before.clone();
        for at in (start..end).step_by(2) {
            literal[at] = 98;
        }
        let after =
            fs::read(second.join(format!("h{header}-alternate-owned-second.accdb"))).unwrap();
        assert_eq!(after, literal);
        read_view(&after, definition, maps, Some("bbbbbbbbbbbbbbbbbbb"));
        memberships(&after, maps, old, new);
        assert_eq!(fs::read(path).unwrap(), before);
        views += 2;
    }
    assert_eq!(views, 4);
}
