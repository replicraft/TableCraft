//! Database settings remain inert observations with exact catalog ownership.
use super::{budget, copy};
use std::io::Cursor;
use tablecraft_ace::{DatabaseQueryMode, InspectError, inspect_database_properties, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    data_page, database_properties as fixture, mutation_fixture, put16, put32, record,
};

#[test]
fn public_sample_database_mode_is_read_without_mutation_or_default_inference() {
    let mut source = copy();
    let original = source.get_ref().clone();
    let mut work = budget();
    let properties =
        inspect_database_properties(&mut source, CatalogScope::new(7), &mut work, &|| false)
            .unwrap()
            .unwrap();
    assert!(properties.raw_bytes().starts_with(b"MR2\0"));
    assert_eq!(
        properties.query_mode(&mut work, &|| false),
        Ok(DatabaseQueryMode::Ansi89)
    );
    assert_eq!(source.get_ref(), &original);
}

#[test]
fn exact_long_zero_one_and_absent_unknown_properties_stay_distinct() {
    for (flags, kind, raw, expected) in [
        (1, 4, &[0, 0, 0, 0][..], DatabaseQueryMode::Ansi89),
        (1, 4, &[1, 0, 0, 0][..], DatabaseQueryMode::Ansi92),
        (1, 4, &[2, 0, 0, 0][..], DatabaseQueryMode::Unknown),
        (1, 4, &[255, 255, 255, 255][..], DatabaseQueryMode::Unknown),
        (0, 4, &[1, 0, 0, 0][..], DatabaseQueryMode::Unknown),
        (0x81, 4, &[1, 0, 0, 0][..], DatabaseQueryMode::Unknown),
        (1, 1, &[1][..], DatabaseQueryMode::Unknown),
    ] {
        let bag = fixture::query_modes(&[(flags, kind, raw)]);
        let bytes = fixture::database(Some(&bag));
        let mut source = Cursor::new(&bytes);
        let mut work = budget();
        let properties =
            inspect_database_properties(&mut source, CatalogScope::new(1), &mut work, &|| false)
                .unwrap()
                .unwrap();
        assert_eq!(properties.query_mode(&mut work, &|| false), Ok(expected));
    }
    for bytes in [
        fixture::database(None),
        fixture::database(Some(&fixture::query_modes(&[]))),
    ] {
        let mut work = budget();
        let properties = inspect_database_properties(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            &mut work,
            &|| false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            properties.query_mode(&mut work, &|| false),
            Ok(DatabaseQueryMode::Absent)
        );
    }
    assert!(
        inspect_database_properties(
            &mut Cursor::new(mutation_fixture::database()),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .is_none()
    );
}

fn slot_start(bytes: &[u8], slot: usize) -> usize {
    let offset = 4 * 4096 + 14 + slot * 2;
    4 * 4096
        + usize::from(u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) & 0x1fff)
}

#[test]
fn malformed_property_frames_and_wrong_database_associations_fail() {
    let bag = fixture::query_modes(&[(1, 4, &[1, 0, 0, 0])]);
    for (slot, field, value, width) in [(3, 10, 1, 2), (4, 10, 1, 2), (4, 6, 999, 4)] {
        let mut bytes = fixture::database(Some(&bag));
        let at = slot_start(&bytes, slot) + field;
        if width == 2 {
            put16(&mut bytes, at, value as u16);
        } else {
            put32(&mut bytes, at, value);
        }
        assert!(
            inspect_database_properties(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
    }
    for bag in [
        fixture::query_modes(&[(1, 4, &[0, 0, 0, 0]), (1, 4, &[1, 0, 0, 0])]),
        b"bad!".to_vec(),
    ] {
        assert!(
            inspect_database_properties(
                &mut Cursor::new(fixture::database(Some(&bag))),
                CatalogScope::new(1),
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
    }
    let malformed = fixture::query_modes(&[(1, 4, &[1, 0])]);
    let mut work = budget();
    let properties = inspect_database_properties(
        &mut Cursor::new(fixture::database(Some(&malformed))),
        CatalogScope::new(1),
        &mut work,
        &|| false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        properties.query_mode(&mut work, &|| false),
        Err(InspectError::Malformed)
    );
}

#[test]
fn database_inspection_and_mode_observation_share_cancellation_and_resource_controls() {
    let bytes = fixture::database(None);
    for mut work in [
        ResourceBudget::new(0, 1000, 1000),
        ResourceBudget::new(16 * 1024 * 1024, 0, 1000),
        ResourceBudget::new(16 * 1024 * 1024, 1000, 0),
    ] {
        assert!(matches!(
            inspect_database_properties(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                &mut work,
                &|| false,
            ),
            Err(InspectError::Limit(_))
        ));
    }
    assert!(matches!(
        inspect_database_properties(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| true,
        ),
        Err(InspectError::Cancelled)
    ));
    let bag = fixture::query_modes(&[(1, 4, &[1, 0, 0, 0])]);
    let properties = inspect_database_properties(
        &mut Cursor::new(fixture::database(Some(&bag))),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        properties.query_mode(&mut budget(), &|| true),
        Err(InspectError::Cancelled)
    );
    assert!(matches!(
        properties.query_mode(&mut ResourceBudget::new(1000, 0, 0), &|| false),
        Err(InspectError::Limit(_))
    ));
}

#[test]
fn duplicate_database_names_and_orphaned_roles_are_not_absent_metadata() {
    let mut bytes = fixture::database(None);
    let mut rows = catalog_rows(&bytes);
    rows.push((0, record(22, 20, 2, 0x80000000, b"\xff\xfeMSysDb")));
    put32(&mut bytes, 2 * 4096 + 16, 6);
    bytes[4 * 4096..5 * 4096].copy_from_slice(&data_page(2, &rows));
    assert!(matches!(
        inspect_database_properties(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::Malformed)
    ));

    let mut bytes = fixture::database(None);
    let at = slot_start(&bytes, 3) + 18;
    bytes[at..at + 9].copy_from_slice(b"Unrelated");
    assert!(matches!(
        inspect_database_properties(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::Malformed)
    ));
}

fn catalog_rows(bytes: &[u8]) -> Vec<(u16, Vec<u8>)> {
    let mut rows = Vec::new();
    let mut end = 4096;
    for slot in 0..5 {
        let start = slot_start(bytes, slot) - 4 * 4096;
        rows.push((0, bytes[4 * 4096 + start..4 * 4096 + end].to_vec()));
        end = start;
    }
    rows
}

#[test]
fn normal_user_table_named_databases_does_not_shadow_the_metadata_container() {
    let bag = fixture::query_modes(&[(1, 4, &[1, 0, 0, 0])]);
    let mut bytes = fixture::database(Some(&bag));
    let mut rows = catalog_rows(&bytes);
    // Rename the existing user table, retaining its actual definition and rows.
    rows[2] = (0, record(10, 0, 1, 0, b"\xff\xfeDatabases"));
    bytes[4 * 4096..5 * 4096].copy_from_slice(&data_page(2, &rows));
    let original = bytes.clone();
    let mut source = Cursor::new(bytes);
    let mut work = budget();
    let properties =
        inspect_database_properties(&mut source, CatalogScope::new(1), &mut work, &|| false)
            .unwrap()
            .unwrap();
    assert_eq!(
        properties.query_mode(&mut work, &|| false),
        Ok(DatabaseQueryMode::Ansi92)
    );

    let table = read_table(
        &mut source,
        CatalogScope::new(1),
        "Databases",
        &mut work,
        &|| false,
    )
    .unwrap();
    assert_eq!(
        table.rows(),
        &[
            vec![Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(2), Value::Int32(20)],
            vec![Value::Int32(3), Value::Int32(30)],
        ]
    );
    assert_eq!(source.get_ref(), &original);
}
