//! Source-only literal INSERT/DELETE lifecycle, before any creator or writer.
use crate::{NativeImage, check_table, read_table};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_native17_sessions;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn literal_native17_session_images_read_complete_rows_before_any_producer() {
    let source = canonical_native17_sessions::source();
    for header in [5_u32, 6] {
        for (raw, inserted) in [
            (&source.before, false),
            (&source.inserted, true),
            (&source.deleted, false),
        ] {
            let mut bytes = raw.clone();
            bytes[20..24].copy_from_slice(&header.to_le_bytes());
            let scope = CatalogScope::new(203);
            let image =
                NativeImage::capture(&mut Cursor::new(&bytes), scope, &mut budget(), &|| false)
                    .unwrap();
            let expected = if inserted {
                vec![
                    vec![Value::Int32(1), Value::Int32(10)],
                    vec![Value::Int32(2), Value::Int32(20)],
                ]
            } else {
                vec![vec![Value::Int32(1), Value::Int32(10)]]
            };
            let rows = read_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(rows.rows(), expected);
            let checked = check_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(checked.rows(), expected.len());
            let captured =
                super::super::normal_ordinary::capture(&image, "Items", &mut budget(), &|| false)
                    .unwrap();
            assert_eq!(captured.table.rows(), expected);
            assert_eq!(captured.records.len(), expected.len());
        }
    }
}

#[test]
#[ignore = "create-new source export only; no creator or mutation producer"]
fn export_literal_native17_session_sources() {
    let source = canonical_native17_sessions::source();
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_NATIVE17_SESSION_EXPORT_DIR").unwrap());
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let mut rows = Vec::new();
    for header in [5_u32, 6] {
        for (id, raw) in [
            ("before", &source.before),
            ("inserted", &source.inserted),
            ("deleted", &source.deleted),
        ] {
            let mut bytes = raw.clone();
            bytes[20..24].copy_from_slice(&header.to_le_bytes());
            let name = format!("h{header}-session-{id}.accdb");
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join(&name))
                .unwrap();
            std::io::Write::write_all(&mut file, &bytes).unwrap();
            let sha = Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            rows.push(format!("{name}\t{}\t{sha}", bytes.len()));
        }
    }
    fs::write(
        directory.join("manifest.tsv"),
        format!("{}\n", rows.join("\n")),
    )
    .unwrap();
}
