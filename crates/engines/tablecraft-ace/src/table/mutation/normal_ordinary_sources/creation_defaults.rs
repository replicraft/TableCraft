//! Original/read-only sources; no creator, default resolver or INSERT planner.
use crate::{
    NativeImage, PropertyGroupKind, check_table, check_table_constraints, inspect_table_properties,
    read_table,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_creation_defaults::{self as sources, Source};

const SCOPE: CatalogScope = CatalogScope::new(219);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn check(bytes: &[u8], rows: &[Vec<Value>], source: &Source) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(table.rows(), rows);
    assert_eq!(
        table
            .schema()
            .columns()
            .iter()
            .map(|column| column.name())
            .collect::<Vec<_>>(),
        ["Id", "Score"]
    );
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), rows.len());
    check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    let properties =
        inspect_table_properties(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(
        properties.raw_bytes(),
        source.property_bytes.as_deref().unwrap_or(&[])
    );
    assert_eq!(
        properties.groups().len(),
        usize::from(source.property_bytes.is_some())
    );
    if source.property_bytes.is_some() {
        crate::properties::framing::validate(&properties, &mut budget(), &|| false).unwrap();
        let group = &properties.groups()[0];
        assert_eq!(group.kind(), PropertyGroupKind::Column);
        assert_eq!(group.name(), "Score");
        assert_eq!(
            group.properties().len(),
            usize::from(source.expression.is_some()) + usize::from(source.required.is_some())
        );
        if let Some(expression) = source.expression {
            let actual = group.get("DefaultValue").unwrap();
            assert_eq!(actual.native_type(), 12);
            assert_eq!(actual.flags(), 1);
            assert_eq!(actual.text().unwrap(), expression);
            let expected = expression
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            assert_eq!(actual.raw_value(), expected.as_slice());
        } else {
            assert!(group.get("DefaultValue").is_none());
        }
        if let Some(required) = source.required {
            let actual = group.get("Required").unwrap();
            assert_eq!(actual.native_type(), 1);
            assert_eq!(actual.flags(), 1);
            assert_eq!(actual.boolean().unwrap(), required);
            assert_eq!(actual.raw_value(), &[if required { 255 } else { 0 }]);
        } else {
            assert!(group.get("Required").is_none());
        }
    }
    let captured =
        super::super::normal_ordinary::capture(&image, "Items", &mut budget(), &|| false).unwrap();
    assert_eq!(captured.table.rows(), rows);
}

#[test]
fn literal_creation_defaults_and_presence_afters_have_complete_read_only_qualification() {
    let mut before = 0;
    let mut after = 0;
    for case in sources::CASES {
        let source = sources::source(case);
        for header in [5_u32, 6] {
            let mut bytes = source.bytes.clone();
            bytes[20..24].copy_from_slice(&header.to_le_bytes());
            check(&bytes, &source.rows, &source);
            before += 1;
            for supplied_null in [false, true] {
                let Some((mut final_bytes, rows)) = sources::inserted(&source, supplied_null)
                else {
                    assert!(source.omitted_refused);
                    continue;
                };
                final_bytes[20..24].copy_from_slice(&header.to_le_bytes());
                check(&final_bytes, &rows, &source);
                assert_eq!(&final_bytes[..7 * 4096], &bytes[..7 * 4096]);
                assert_eq!(
                    &final_bytes[7 * 4096..7 * 4096 + 16],
                    &bytes[7 * 4096..7 * 4096 + 16]
                );
                assert_eq!(
                    &final_bytes[7 * 4096 + 20..8 * 4096],
                    &bytes[7 * 4096 + 20..8 * 4096]
                );
                let data_base = 8 * 4096;
                let original_count = source.rows.len();
                let new_slot = data_base + 14 + 2 * original_count;
                let new_start = data_base
                    + usize::from(u16::from_le_bytes(
                        final_bytes[new_slot..new_slot + 2].try_into().unwrap(),
                    ));
                for at in 0..bytes.len() {
                    let changed = (7 * 4096 + 16..7 * 4096 + 20).contains(&at)
                        || (data_base + 2..data_base + 4).contains(&at)
                        || (data_base + 12..data_base + 14).contains(&at)
                        || (new_slot..new_slot + 2).contains(&at)
                        || (new_start..new_start + 11).contains(&at);
                    if !changed {
                        assert_eq!(final_bytes[at], bytes[at], "protected source byte {at}");
                    }
                }
                for slot in 0..original_count {
                    let at = 8 * 4096 + 14 + 2 * slot;
                    assert_eq!(&final_bytes[at..at + 2], &bytes[at..at + 2]);
                }
                let new = rows.last().unwrap();
                assert_eq!(new[0], Value::Int32(if supplied_null { 25 } else { 24 }));
                assert_eq!(
                    new[1],
                    if supplied_null {
                        Value::Null
                    } else {
                        source.omitted_score.map_or(Value::Null, Value::Int32)
                    }
                );
                after += 1;
            }
        }
    }
    assert_eq!((before, after), (12, 20));
}

#[test]
#[ignore = "explicit create-new source export; no creator, resolver or mutation"]
fn export_literal_creation_default_sources() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_CREATION_DEFAULTS_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let mut manifest = String::new();
    for case in sources::CASES {
        let source = sources::source(case);
        for header in [5_u32, 6] {
            let mut views = vec![("before", source.bytes.clone())];
            for (phase, supplied_null) in [("omitted", false), ("supplied-null", true)] {
                if let Some((bytes, _)) = sources::inserted(&source, supplied_null) {
                    views.push((phase, bytes));
                }
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
