//! Larger original inputs, checked without ordinary mutation or certification.
use super::{SCOPE, budget, export, model as ordinary};
use crate::{NativeImage, check_table, check_table_constraints, read_table};
use sha2::{Digest, Sha256};
use std::{env, fs, io::Cursor, io::Write, path::Path};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::canonical_ordinary_inline::{self as model, Source};

fn source(header: u8, case: model::Case) -> Source {
    let mut source = model::source(case);
    source.bytes[20..24].copy_from_slice(&u32::from(header).to_le_bytes());
    assert_eq!(source.bytes.len(), 96 * 4096);
    source
}

#[test]
fn ten_larger_ordinary_sources_match_literal_rows_and_constraints() {
    let mut views = 0;
    for header in [5, 6] {
        for case in model::CASES {
            let source = source(header, case);
            if !source.ordinary {
                continue;
            }
            let image = NativeImage::capture(
                &mut Cursor::new(&source.bytes),
                SCOPE,
                &mut budget(),
                &|| false,
            )
            .unwrap_or_else(|error| panic!("{}: {error}", case.id()));
            let checked = check_table(
                &mut image.reader(),
                SCOPE,
                source.table,
                &mut budget(),
                &|| false,
            )
            .unwrap_or_else(|error| panic!("{}: {error}", case.id()));
            assert_eq!(checked.rows(), source.rows.len());
            assert_eq!(checked.schema().definition_page(), source.definition);
            assert!(checked.indexes().is_empty());

            let table = read_table(
                &mut image.reader(),
                SCOPE,
                source.table,
                &mut budget(),
                &|| false,
            )
            .unwrap_or_else(|error| panic!("{}: {error}", case.id()));
            let expected = source
                .rows
                .iter()
                .map(|row| row.iter().copied().map(Value::Int32).collect::<Vec<_>>())
                .collect::<Vec<_>>();
            assert_eq!(table.rows(), expected, "{}", case.id());
            let constraints = check_table_constraints(
                &mut image.reader(),
                SCOPE,
                source.table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(constraints.rows(), source.rows.len());
            assert!(constraints.relationships().is_empty());
            assert_eq!(image.as_bytes(), source.bytes);
            views += 1;
        }
    }
    assert_eq!(views, 10);
}

fn create(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

#[test]
#[ignore = "explicit fresh absolute directory; no automatic source export"]
fn export_larger_ordinary_sources_and_literal_expectations() {
    let directory = env::var_os("TABLECRAFT_ORDINARY_INLINE_SOURCE_DIR").expect("source directory");
    let directory = Path::new(&directory);
    assert!(directory.is_absolute() && !directory.exists());
    fs::create_dir(directory).unwrap();
    let mut inventory = String::from("id\tordinary\tbytes\tsha256\n");
    for header in [5, 6] {
        for case in model::CASES {
            let source = source(header, case);
            let id = format!("h{header}-{}", case.id());
            let digest = Sha256::digest(&source.bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            create(&directory.join(format!("{id}.accdb")), &source.bytes);
            inventory.push_str(&format!(
                "{id}\t{}\t{}\t{digest}\n",
                source.ordinary,
                source.bytes.len()
            ));
            if source.ordinary {
                let converted = ordinary::Source {
                    bytes: source.bytes,
                    tables: vec![ordinary::Table {
                        name: source.table.into(),
                        columns: source.columns,
                        rows: source
                            .rows
                            .into_iter()
                            .map(|row| row.into_iter().map(Value::Int32).collect())
                            .collect(),
                        definition: source.definition,
                        data: vec![source.data],
                    }],
                    ordinary: true,
                };
                create(
                    &directory.join(format!("{id}.expected.tsv")),
                    export::expected(&converted, header, &digest).as_bytes(),
                );
            }
        }
    }
    create(&directory.join("inventory.tsv"), inventory.as_bytes());
    fs::File::open(directory).unwrap().sync_all().unwrap();
}
