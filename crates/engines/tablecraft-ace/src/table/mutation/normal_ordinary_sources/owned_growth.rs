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
#[ignore = "requires explicit pinned originals and independent owned-growth expectations"]
fn genuine_sentinel_owned_growth_sources_read_before_any_new_writer() {
    let originals =
        PathBuf::from(std::env::var_os("TABLECRAFT_PROPERTY_LVAL_ORIGINALS_DIR").unwrap());
    let expected = PathBuf::from(std::env::var_os("TABLECRAFT_OWNED_GROWTH_EXPECTED_DIR").unwrap());
    assert!(originals.is_absolute() && expected.is_absolute());
    let mut views = 0;
    for (header, length, digest, definition, maps, data) in [
        (
            5_u8,
            458752,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            107,
            108,
            110_usize,
        ),
        (
            6_u8,
            528384,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            125,
            126,
            127_usize,
        ),
    ] {
        let path = originals.join(format!("h{header}.accdb"));
        let before = fs::read(&path).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(hash(&before), digest);
        read_view(&before, definition, maps, Some("unchanged sentinel"));
        let after =
            fs::read(expected.join(format!("h{header}-marker-owned-growth.accdb"))).unwrap();
        let base = data * 4096;
        let original = &before[base + 4030..base + 4096];
        assert_eq!(&original[..6], &[3, 0, 23, 0, 0, 0]);
        assert_eq!(
            &original[42..57],
            &[3, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 199, 0, 255]
        );
        let mut terminal = original[..6].to_vec();
        for _ in 0..19 {
            terminal.extend_from_slice(&[97, 0]);
        }
        terminal.extend_from_slice(&original[42..57]);
        terminal.extend_from_slice(&[59, 0, 44, 0, 6, 0, 2, 0, 7]);
        assert_eq!(terminal.len(), 68);
        let mut literal = before.clone();
        literal[base + 3962..base + 4030].copy_from_slice(&terminal);
        literal[base + 4030..base + 4096].fill(0);
        literal[base + 4030..base + 4034]
            .copy_from_slice(&(((data as u32) << 8) | 1).to_le_bytes());
        literal[base + 14..base + 16].copy_from_slice(&[190, 79]);
        literal[base + 16..base + 18].copy_from_slice(&[122, 143]);
        literal[base + 12..base + 14].copy_from_slice(&[2, 0]);
        literal[base + 2..base + 4].copy_from_slice(&[104, 15]);
        assert_eq!(after, literal);
        assert_eq!(&after[..base], &before[..base]);
        assert_eq!(&after[base + 4096..], &before[base + 4096..]);
        assert_eq!(
            &after[base + 18..base + 3962],
            &before[base + 18..base + 3962]
        );
        read_view(&after, definition, maps, Some("aaaaaaaaaaaaaaaaaaa"));
        assert_eq!(fs::read(path).unwrap(), before);
        views += 2;
    }
    assert_eq!(views, 4);
}
