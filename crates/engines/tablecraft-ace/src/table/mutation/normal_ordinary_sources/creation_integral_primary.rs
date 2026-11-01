//! Source-only manual primary declarations, original rows and literal INSERTs.
use crate::{
    IndexCheckStatus, IndexConstraintViolation, InspectError, NativeImage, check_table,
    check_table_constraints, inspect_table_index_counter, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_creation_integral_primary::{self as sources, Violation};

const SCOPE: CatalogScope = CatalogScope::new(222);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn checked(bytes: &[u8], rows: &[Vec<Value>], history: u32, family: sources::Family) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    assert_eq!(bytes.len(), 10 * 4096);
    let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(table.rows(), rows);
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), rows.len());
    assert_eq!(checked.indexes().len(), 1);
    assert_eq!(checked.indexes()[0].status(), IndexCheckStatus::Checked);
    let index = checked.indexes()[0].index();
    assert_eq!(index.name(), "PrimaryId");
    assert_eq!(index.number(), 0);
    assert_eq!(index.data_number(), 0);
    assert_eq!(index.native_type(), 1);
    assert_eq!(index.flags(), 137);
    assert_eq!(index.root_page(), 9);
    assert_eq!(index.usage_map(), (3 << 8) | 22);
    assert_eq!(index.columns().len(), 1);
    assert_eq!(index.columns()[0].number(), 0);
    assert!(!index.columns()[0].descending());
    for (position, field) in checked.schema().columns().iter().enumerate() {
        assert_eq!(
            field.native_type(),
            if position == 0 {
                family.native_type()
            } else {
                4
            }
        );
        assert_eq!(field.storage_length(), if position == 0 { 8 } else { 4 });
        assert_eq!(field.flags(), 3);
        assert_eq!(field.extended_flags(), 0);
    }
    assert_eq!(&bytes[7 * 4096 + 20..7 * 4096 + 24], &[0; 4]);
    let count = inspect_table_index_counter(
        &mut image.reader(),
        SCOPE,
        "Items",
        "PrimaryId",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(count.value(), history);
    let mut expected = rows.to_vec();
    expected.sort_by_key(|row| match row[0] {
        Value::Int64(value) => value,
        Value::Currency(value) => value.scaled(),
        _ => panic!("healthy present key"),
    });
    let ordered = read_table_index(
        &mut image.reader(),
        SCOPE,
        "Items",
        "PrimaryId",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(ordered.rows(), expected);
    check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    let captured =
        super::super::normal_ordinary::capture(&image, "Items", &mut budget(), &|| false).unwrap();
    assert_eq!(captured.table.rows(), rows);
}

#[test]
fn literal_wide_primary_creation_insert_and_delete_views_preserve_constraints_and_ownership() {
    let mut before = 0;
    let mut after = 0;
    let mut refused = 0;
    let mut deleted = 0;
    for case in sources::CASES {
        let source = sources::source(case);
        for header in [5_u32, 6] {
            let mut bytes = source.bytes.clone();
            bytes[20..24].copy_from_slice(&header.to_le_bytes());
            if let Some(violation) = source.violation {
                let image =
                    NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
                        .unwrap();
                let rows = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                    false
                })
                .unwrap();
                assert_eq!(rows.rows(), source.rows);
                let expected = match violation {
                    Violation::DuplicateKey => IndexConstraintViolation::DuplicateKey,
                    Violation::NullKey => IndexConstraintViolation::NullKey,
                };
                assert!(
                    matches!(check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| false),
                                 Err(InspectError::IndexConstraint(actual)) if actual == expected)
                );
                assert!(
                    super::super::normal_ordinary::capture(&image, "Items", &mut budget(), &|| {
                        false
                    })
                    .is_err()
                );
                assert!(sources::inserted(&source).is_none());
                refused += 1;
            } else {
                checked(&bytes, &source.rows, source.history, source.family);
                let (mut final_bytes, rows) = sources::inserted(&source).unwrap();
                final_bytes[20..24].copy_from_slice(&header.to_le_bytes());
                checked(&final_bytes, &rows, source.history + 1, source.family);
                assert_eq!(&final_bytes[..7 * 4096], &bytes[..7 * 4096]);
                assert_eq!(
                    &final_bytes[7 * 4096..7 * 4096 + 16],
                    &bytes[7 * 4096..7 * 4096 + 16]
                );
                assert_eq!(
                    &final_bytes[7 * 4096 + 20..7 * 4096 + 67],
                    &bytes[7 * 4096 + 20..7 * 4096 + 67]
                );
                assert_eq!(
                    &final_bytes[7 * 4096 + 71..8 * 4096],
                    &bytes[7 * 4096 + 71..8 * 4096]
                );
                let original_data = &bytes[8 * 4096..9 * 4096];
                let inserted_data = &final_bytes[8 * 4096..9 * 4096];
                assert_eq!(&original_data[..2], &inserted_data[..2]);
                assert_eq!(&original_data[4..12], &inserted_data[4..12]);
                assert_eq!(
                    &original_data[14..14 + 2 * source.rows.len()],
                    &inserted_data[14..14 + 2 * source.rows.len()],
                );
                let original_frontier = 4096 - 15 * source.rows.len();
                assert_eq!(
                    &original_data[original_frontier..],
                    &inserted_data[original_frontier..],
                );
                let (mut removed, remaining) = sources::deleted_after_insert(&source).unwrap();
                removed[20..24].copy_from_slice(&header.to_le_bytes());
                checked(&removed, &remaining, source.history + 1, source.family);
                let selected = 8 * 4096 + 14 + 2 * source.rows.len();
                let mut expected = final_bytes.clone();
                expected[7 * 4096 + 16..7 * 4096 + 20]
                    .copy_from_slice(&(source.rows.len() as u32).to_le_bytes());
                expected[selected + 1] |= 0xc0;
                // Independent original survivor leaf overlays only active bytes,
                // bitmap and free; vacated inserted tail remains immutable.
                let free =
                    u16::from_le_bytes(bytes[9 * 4096 + 2..9 * 4096 + 4].try_into().unwrap());
                let active = 3616 - usize::from(free);
                expected[9 * 4096 + 2..9 * 4096 + 4]
                    .copy_from_slice(&bytes[9 * 4096 + 2..9 * 4096 + 4]);
                expected[9 * 4096 + 27..9 * 4096 + 480 + active]
                    .copy_from_slice(&bytes[9 * 4096 + 27..9 * 4096 + 480 + active]);
                assert_eq!(removed, expected);
                after += 1;
                deleted += 1;
            }
            before += 1;
        }
    }
    assert_eq!((before, after, deleted, refused), (16, 8, 8, 8));
}

#[test]
#[ignore = "explicit create-new manual primary SOURCE export; no creator or mutation"]
fn export_literal_wide_primary_creation_sources() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_CREATION_INTEGRAL_PRIMARY_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let mut manifest = String::new();
    for case in sources::CASES {
        let source = sources::source(case);
        for header in [5_u32, 6] {
            let mut views = vec![("before", source.bytes.clone())];
            if let Some((bytes, _)) = sources::inserted(&source) {
                views.push(("inserted", bytes));
                views.push(("deleted", sources::deleted_after_insert(&source).unwrap().0));
            }
            for (phase, mut bytes) in views {
                bytes[20..24].copy_from_slice(&header.to_le_bytes());
                let name = format!("h{header}-{}-{phase}.accdb", case.id());
                let mut output = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(directory.join(&name))
                    .unwrap();
                output.write_all(&bytes).unwrap();
                let sha = Sha256::digest(&bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                manifest.push_str(&format!("{name}\t{}\t{sha}\n", bytes.len()));
            }
        }
    }
    assert_eq!(manifest.lines().count(), 32);
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("manifest.tsv"))
        .unwrap();
    output.write_all(manifest.as_bytes()).unwrap();
}
