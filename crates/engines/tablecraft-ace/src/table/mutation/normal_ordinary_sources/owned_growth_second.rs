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
#[ignore = "requires independently constructed first and second forwarded source images"]
fn genuine_forwarded_second_edit_sources_read_before_any_writer() {
    let first = PathBuf::from(std::env::var_os("TABLECRAFT_OWNED_GROWTH_EXPECTED_DIR").unwrap());
    let second =
        PathBuf::from(std::env::var_os("TABLECRAFT_OWNED_GROWTH_SECOND_EXPECTED_DIR").unwrap());
    assert!(first.is_absolute() && second.is_absolute());
    let mut views = 0;
    for (header, digest, definition, maps, data) in [
        (
            5_u8,
            "2bfdbfc9caddaa5151bbd3006f095e49c4afeba02d0d5ac9b4ffeb84439ce17e",
            107,
            108,
            110_usize,
        ),
        (
            6_u8,
            "4cd5c2f9ca8c65e2cb74ec0269712adb8662b0457af6ecce121743dcec1a5775",
            125,
            126,
            127_usize,
        ),
    ] {
        let first_path = first.join(format!("h{header}-marker-owned-growth.accdb"));
        let before = fs::read(&first_path).unwrap();
        assert_eq!(hash(&before), digest);
        read_view(&before, definition, maps, Some("aaaaaaaaaaaaaaaaaaa"));
        let after =
            fs::read(second.join(format!("h{header}-marker-forwarded-second.accdb"))).unwrap();
        let start = data * 4096 + 3968;
        let end = start + 38;
        assert_eq!(
            &before[start..end],
            b"a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0a\0"
        );
        let mut literal = before.clone();
        for at in (start..end).step_by(2) {
            literal[at] = b'b';
        }
        assert_eq!(after, literal);
        assert_eq!(&after[..start], &before[..start]);
        assert_eq!(&after[end..], &before[end..]);
        read_view(&after, definition, maps, Some("bbbbbbbbbbbbbbbbbbb"));
        assert_eq!(fs::read(first_path).unwrap(), before);
        views += 2;
    }
    assert_eq!(views, 4);
}
