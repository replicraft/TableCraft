//! Explicit original/expected reads only; no mutation or permission API is called.
use crate::{
    LongValueCheckStatus, NativeImage, PayloadPresence, PropertyGroupKind, check_table_long_values,
    inspect_table_indexes, inspect_table_properties, read_table,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(133);
const OLD: &str = "unchanged sentinel";
const RECIPES: [(&str, Option<&str>); 6] = [
    ("same", Some("qualified sentinel")),
    ("shorter", Some("changed sentinel")),
    ("empty", Some("")),
    ("clear", None),
    ("noop", Some(OLD)),
    ("empty-group", Some(OLD)),
];
const SOURCES: [(u8, usize, &str, u32, u32, u32); 2] = [
    (
        5,
        458752,
        "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
        107,
        108,
        110,
    ),
    (
        6,
        528384,
        "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
        125,
        126,
        127,
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

fn literal(before: &[u8], data: u32, marker: Option<&str>) -> Vec<u8> {
    let mut after = before.to_vec();
    let start = data as usize * PAGE + 4030;
    let original = &before[start..start + 66];
    let original_marker = OLD
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    assert_eq!(&original[6..42], original_marker.as_slice());
    assert_eq!(
        &original[42..57],
        &[3, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 199, 0, 255]
    );
    let replacement = marker
        .unwrap_or("")
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    assert!(replacement.len() <= 36);
    let row = &mut after[start..start + 66];
    row[6..6 + replacement.len()].copy_from_slice(&replacement);
    row[6 + replacement.len()..21 + replacement.len()].copy_from_slice(&original[42..57]);
    for (at, offset) in [
        (57, 21 + replacement.len()),
        (59, 6 + replacement.len()),
        (61, 6),
    ] {
        row[at..at + 2].copy_from_slice(&(offset as u16).to_le_bytes());
    }
    row[65] = (original[65] & !2) | if marker.is_some() { 2 } else { 0 };
    after
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

#[test]
#[ignore = "requires explicit original and independently constructed expected directories"]
fn genuine_marker_property_and_readonly_ole_views_qualify_before_any_writer() {
    let originals =
        PathBuf::from(std::env::var_os("TABLECRAFT_PROPERTY_LVAL_ORIGINALS_DIR").unwrap());
    let expected =
        PathBuf::from(std::env::var_os("TABLECRAFT_PROPERTY_LVAL_EXPECTED_DIR").unwrap());
    assert!(originals.is_absolute() && expected.is_absolute());
    let mut views = 0;
    for (header, length, digest, definition, maps, data) in SOURCES {
        let before = fs::read(originals.join(format!("h{header}.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(hash(&before), digest);
        read_view(&before, definition, maps, Some(OLD));
        views += 1;
        for (recipe, marker) in RECIPES {
            let after =
                fs::read(expected.join(format!("h{header}-marker-{recipe}.accdb"))).unwrap();
            assert_eq!(after, literal(&before, data, marker));
            read_view(&after, definition, maps, marker);
            views += 1;
        }
        assert_eq!(
            fs::read(originals.join(format!("h{header}.accdb"))).unwrap(),
            before
        );
    }
    assert_eq!(views, 14);
}
