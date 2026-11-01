//! Original-only literal DELETE source reads; no delete planner or journal calls.
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
            Value::Text("unchanged sentinel".into()),
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
#[ignore = "requires explicit independently constructed ordinary DELETE source directory"]
fn original_only_delete_source_views_preserve_payloads_and_read_zero_live_rows() {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_NORMAL_DELETE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    for (header, length, before_sha, after_sha) in [
        (
            5,
            458752,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            "187714c3125969f8b47a50b1e83d993f3512fe21911bb0043e650b7dcd2754da",
        ),
        (
            6,
            528384,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            "de8da4c54fb46f384102900212376ca1c91768bdf8e6b2c153e4410e844ab03e",
        ),
    ] {
        let (definition, maps, data) = match header {
            5 => (107, 108, 110),
            6 => (125, 126, 127),
            _ => unreachable!(),
        };
        let before =
            fs::read(directory.join(format!("h{header}-ordinary-delete-before.accdb"))).unwrap();
        let after =
            fs::read(directory.join(format!("h{header}-ordinary-delete-deleted.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(hash(&before), before_sha);
        assert_eq!(hash(&after), after_sha);
        assert_eq!(before.len(), after.len());
        read_view(&before, definition, maps, false);
        read_view(&after, definition, maps, true);

        let count = definition as usize * 4096 + 16;
        let slot = data * 4096 + 14;
        assert_eq!(&before[count..count + 4], &[1, 0, 0, 0]);
        assert_eq!(&after[count..count + 4], &[0, 0, 0, 0]);
        assert_eq!(&before[slot..slot + 2], &[190, 15]);
        assert_eq!(&after[slot..slot + 2], &[190, 207]);
        for at in 0..before.len() {
            if !(count..count + 4).contains(&at) && !(slot..slot + 2).contains(&at) {
                assert_eq!(
                    before[at], after[at],
                    "outside literal DELETE spans at {at}"
                );
            }
        }
        assert_eq!(
            &before[data * 4096 + 4030..(data + 1) * 4096],
            &after[data * 4096 + 4030..(data + 1) * 4096]
        );
    }
}
