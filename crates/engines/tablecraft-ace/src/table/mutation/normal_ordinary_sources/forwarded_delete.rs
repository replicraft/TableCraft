//! Independent EOF-forwarded literal DELETE source reads; no delete planner or journal calls.
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

fn read_view(bytes: &[u8], definition: u32, maps: u32, deleted: bool) {
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
    let expected = if deleted {
        Vec::new()
    } else {
        vec![vec![
            Value::Int32(23),
            Value::Text("aaaaaaaaaaaaaaaaaaa".into()),
            Value::Binary(vec![199, 0, 255]),
        ]]
    };
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
    assert_eq!(lval.rows(), usize::from(!deleted));
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
        (0, if deleted { 0 } else { 1 }, 0, 0)
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
#[ignore = "requires explicit independently constructed forwarded DELETE source directory"]
fn forwarded_delete_source_views_preserve_terminal_and_read_zero_live_rows() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_FORWARDED_DELETE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    for (header, length, before_sha, definition, maps, data, target) in [
        (
            5,
            462848,
            "3a95a4ecc876ea3cc88990cd83309a051515ddad1b698414cb6c51b53d5841db",
            107,
            108,
            110,
            112,
        ),
        (
            6,
            532480,
            "0db9f897700066ef36bf9acfc90f3f29eda234012ee09515a876bdea3f698b50",
            125,
            126,
            127,
            129,
        ),
    ] {
        let before =
            fs::read(directory.join(format!("h{header}-forwarded-delete-before.accdb"))).unwrap();
        let after =
            fs::read(directory.join(format!("h{header}-forwarded-delete-deleted.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(after.len(), length);
        assert_eq!(hash(&before), before_sha);
        read_view(&before, definition, maps, false);
        read_view(&after, definition, maps, true);

        let count = definition as usize * 4096 + 16;
        let slot = data * 4096 + 14;
        assert_eq!(&before[count..count + 4], &[1, 0, 0, 0]);
        assert_eq!(&after[count..count + 4], &[0, 0, 0, 0]);
        assert_eq!(&before[slot..slot + 2], &[190, 79]);
        assert_eq!(&after[slot..slot + 2], &[190, 207]);
        let pointer = data * 4096 + 4030;
        assert_eq!(
            &before[pointer..pointer + 4],
            &((target as u32) << 8).to_le_bytes()
        );
        assert!(
            before[pointer + 4..pointer + 66]
                .iter()
                .all(|byte| *byte == 0)
        );
        assert_eq!(
            &before[target * 4096..(target + 1) * 4096],
            &after[target * 4096..(target + 1) * 4096]
        );
        for at in 0..length {
            if !(count..count + 4).contains(&at) && !(slot..slot + 2).contains(&at) {
                assert_eq!(
                    before[at], after[at],
                    "outside literal forwarded DELETE spans at {at}"
                );
            }
        }
    }
}
