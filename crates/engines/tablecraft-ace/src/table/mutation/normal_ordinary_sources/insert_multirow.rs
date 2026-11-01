//! Original-only literal multirow INSERT source reads; no insert planner or journal calls.
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

fn read_view(bytes: &[u8], definition: u32, maps: u32, order: Option<&str>) {
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
    if let Some(order) = order {
        let new = vec![Value::Int32(24), Value::Text("new".into()), Value::Null];
        let tail = vec![Value::Int32(25), Value::Text("tail".into()), Value::Null];
        match order {
            "new-tail" => expected.extend([new, tail]),
            "tail-new" => expected.extend([tail, new]),
            _ => panic!("unexpected literal request order"),
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
    assert_eq!(lval.rows(), if order.is_some() { 3 } else { 1 });
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
        (if order.is_some() { 2 } else { 0 }, 1, 0, 0)
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
#[ignore = "requires independently constructed multirow ordinary INSERT source directory"]
fn multirow_insert_source_views_read_literal_order_and_preserve_original_bytes() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_NORMAL_MULTIROW_INSERT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut seen = Vec::new();
    for (header, order, definition, maps, data, length, original_sha, after_sha) in [
        (
            5_u8,
            "new-tail",
            107,
            108,
            110,
            458752,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            "db9defca73f6013038d2608b9b8e62dd8355d1483039d3a46edad91612fa99ed",
        ),
        (
            5_u8,
            "tail-new",
            107,
            108,
            110,
            458752,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            "fa9eae3cbe20c9742c4f165ee84f510ff8e25b0437a76828bd95cd02b300d908",
        ),
        (
            6_u8,
            "new-tail",
            125,
            126,
            127,
            528384,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            "48b1070f78f68007c50f1409d3b013cdb2140495b1117035be6b847eb3f3d150",
        ),
        (
            6_u8,
            "tail-new",
            125,
            126,
            127,
            528384,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            "a9a64d255718d89ea4c9ada2ce020984ee0742305a7f821c4dd805d19d4b3f32",
        ),
    ] {
        seen.push((header, order.to_owned()));
        let before =
            fs::read(directory.join(format!("h{header}-multirow-{order}-before.accdb"))).unwrap();
        let after =
            fs::read(directory.join(format!("h{header}-multirow-{order}-inserted.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(hash(&after), after_sha);
        assert_eq!(hash(&before), original_sha);
        assert_eq!(before.len(), after.len());
        read_view(&before, definition, maps, None);
        read_view(&after, definition, maps, Some(order));

        let base = data * 4096;
        let count = definition as usize * 4096 + 16;
        assert_eq!(&before[count..count + 4], &[1, 0, 0, 0]);
        assert_eq!(&after[count..count + 4], &[3, 0, 0, 0]);
        assert_eq!(&before[base + 2..base + 4], &[174, 15]);
        assert_eq!(&after[base + 2..base + 4], &[126, 15]);
        assert_eq!(&before[base + 12..base + 14], &[1, 0]);
        assert_eq!(&after[base + 12..base + 14], &[3, 0]);
        assert_eq!(&before[base + 14..base + 20], &[190, 15, 0, 0, 0, 0]);
        let new: &[u8] = &[
            3, 0, 24, 0, 0, 0, 110, 0, 101, 0, 119, 0, 12, 0, 12, 0, 6, 0, 2, 0, 3,
        ];
        let tail: &[u8] = &[
            3, 0, 25, 0, 0, 0, 116, 0, 97, 0, 105, 0, 108, 0, 14, 0, 14, 0, 6, 0, 2, 0, 3,
        ];
        assert_eq!(&before[base + 3986..base + 4030], &[0; 44]);
        match order {
            "new-tail" => {
                assert_eq!(&after[base + 14..base + 20], &[190, 15, 169, 15, 146, 15]);
                assert_eq!(&after[base + 4009..base + 4030], new);
                assert_eq!(&after[base + 3986..base + 4009], tail);
            }
            "tail-new" => {
                assert_eq!(&after[base + 14..base + 20], &[190, 15, 167, 15, 146, 15]);
                assert_eq!(&after[base + 4007..base + 4030], tail);
                assert_eq!(&after[base + 3986..base + 4007], new);
            }
            _ => unreachable!(),
        }
        let allowed = [
            count..count + 4,
            base + 2..base + 4,
            base + 12..base + 14,
            base + 16..base + 20,
            base + 3986..base + 4030,
        ];
        for at in 0..before.len() {
            if !allowed.iter().any(|span| span.contains(&at)) {
                assert_eq!(
                    before[at], after[at],
                    "outside literal multirow spans at {at}"
                );
            }
        }
    }
    seen.sort();
    assert_eq!(
        seen,
        vec![
            (5, "new-tail".to_owned()),
            (5, "tail-new".to_owned()),
            (6, "new-tail".to_owned()),
            (6, "tail-new".to_owned()),
        ]
    );
}
