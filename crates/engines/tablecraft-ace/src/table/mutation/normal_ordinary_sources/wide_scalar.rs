//! Read-only independent full images; no insertion producer or intent encoder.
use crate::{NativeImage, check_table, read_table};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_ordinary_wide_insert as sources;

const SCOPE: CatalogScope = CatalogScope::new(215);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn wide_scalar_originals_and_literal_afters_read_complete_exact_values() {
    let mut views = 0;
    for case in sources::CASES {
        let source = sources::source(case);
        for header in [5_u32, 6] {
            for (after, original) in [(false, &source.before), (true, &source.after)] {
                let mut bytes = original.clone();
                bytes[20..24].copy_from_slice(&header.to_le_bytes());
                let mut rows = source.rows_before.clone();
                if after {
                    rows.push(source.values.clone());
                }
                let image =
                    NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
                        .unwrap();
                let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                    false
                })
                .unwrap();
                assert_eq!(table.rows(), rows, "{} h{header} after={after}", case.id());
                assert_eq!(
                    table
                        .schema()
                        .columns()
                        .iter()
                        .map(|field| field.name())
                        .collect::<Vec<_>>(),
                    source.columns,
                );
                let checked =
                    check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                        false
                    })
                    .unwrap();
                assert_eq!(checked.rows(), rows.len());
                let captured =
                    super::super::normal_ordinary::capture(&image, "Items", &mut budget(), &|| {
                        false
                    })
                    .unwrap();
                assert_eq!(captured.table.rows(), rows);
                assert_eq!(captured.records.len(), rows.len());
                views += 1;
            }
        }
    }
    assert_eq!(views, 32);
}

#[test]
#[ignore = "explicit create-new independent source export; no native writer"]
fn export_literal_wide_scalar_insert_sources() {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_WIDE_INSERT_EXPORT_DIR").unwrap());
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let mut manifest = String::new();
    for case in sources::CASES {
        let source = sources::source(case);
        for header in [5_u32, 6] {
            for (phase, original) in [("before", &source.before), ("after", &source.after)] {
                let mut bytes = original.clone();
                bytes[20..24].copy_from_slice(&header.to_le_bytes());
                let name = format!("h{header}-{}-{phase}.accdb", case.id());
                let mut output = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(directory.join(&name))
                    .unwrap();
                output.write_all(&bytes).unwrap();
                let hash = Sha256::digest(&bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                manifest.push_str(&format!("{name}\t{}\t{hash}\n", bytes.len()));
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
