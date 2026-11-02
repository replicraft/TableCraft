//! Inert source qualification only; no generated request, creator or planner.
use crate::{
    IndexCheckStatus, NativeImage, PropertyGroupKind, check_table, inspect_table_index_counter,
    inspect_table_properties, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_normal_autonumber as sources;

const SCOPE: CatalogScope = CatalogScope::new(231);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn increment_counter_and_random_default_sources_are_read_without_generation() {
    let mut views = 0;
    for case in sources::CASES {
        let source = sources::source(case);
        for header in [5_u32, 6] {
            let mut bytes = source.bytes.clone();
            bytes[20..24].copy_from_slice(&header.to_le_bytes());
            let image =
                NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let rows = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(rows.rows(), source.rows);
            let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(checked.rows(), source.rows.len());
            assert_eq!(checked.schema().columns().len(), 2);
            for (position, field) in checked.schema().columns().iter().enumerate() {
                assert_eq!(field.native_type(), 4);
                assert_eq!(field.storage_length(), 4);
                assert_eq!(field.field.offset, position * 4);
                assert_eq!(field.flags(), if position == 0 { 7 } else { 3 });
                assert_eq!(field.extended_flags(), 0);
            }
            assert_eq!(checked.indexes().len(), 1);
            assert_eq!(checked.indexes()[0].status(), IndexCheckStatus::Checked);
            let ordered = read_table_index(
                &mut image.reader(),
                SCOPE,
                "Items",
                "PrimaryId",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(ordered.rows(), source.rows);
            let history = inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                "Items",
                "PrimaryId",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(history.value(), source.rows.len() as u32);
            assert_eq!(
                &bytes[7 * 4096 + 20..7 * 4096 + 24],
                source.last.to_le_bytes()
            );
            assert_eq!(&bytes[7 * 4096 + 24..7 * 4096 + 28], &[1, 0, 0, 0]);

            let properties = inspect_table_properties(
                &mut image.reader(),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            if let Some(expression) = source.expression {
                crate::properties::framing::validate(&properties, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(properties.groups().len(), 1);
                assert_eq!(properties.raw_bytes().len(), 60 + 2 * expression.len());
                let group = &properties.groups()[0];
                assert_eq!(group.kind(), PropertyGroupKind::Column);
                assert_eq!(group.name(), "Id");
                assert_eq!(group.properties().len(), 1);
                let property = group.get("DefaultValue").unwrap();
                assert_eq!((property.native_type(), property.flags()), (12, 1));
                assert_eq!(property.text().unwrap(), expression);
                assert_eq!(
                    property.raw_value(),
                    expression
                        .encode_utf16()
                        .flat_map(u16::to_le_bytes)
                        .collect::<Vec<_>>()
                );
            } else {
                assert!(properties.raw_bytes().is_empty());
                assert!(properties.groups().is_empty());
                let captured =
                    super::super::normal_actions::capture(&image, "Items", &mut budget(), &|| {
                        false
                    })
                    .unwrap();
                assert_eq!(captured.table.rows(), source.rows);
                assert_eq!(captured.records.len(), source.rows.len());
            }

            let data = &bytes[8 * 4096..9 * 4096];
            assert_eq!(
                u16::from_le_bytes(data[12..14].try_into().unwrap()) as usize,
                source.rows.len()
            );
            for (slot, row) in source.rows.iter().enumerate() {
                let start = 4096 - 11 * (slot + 1);
                assert_eq!(
                    &data[14 + 2 * slot..16 + 2 * slot],
                    (start as u16).to_le_bytes()
                );
                assert_eq!(&data[start..start + 2], &[2, 0]);
                let tablecraft_core::values::Value::Int32(identity) = row[0] else {
                    panic!()
                };
                let tablecraft_core::values::Value::Int32(score) = row[1] else {
                    panic!()
                };
                assert_eq!(&data[start + 2..start + 6], identity.to_le_bytes());
                assert_eq!(&data[start + 6..start + 10], score.to_le_bytes());
                assert_eq!(data[start + 10], 3);
            }
            assert_eq!(
                source.inserted,
                matches!(
                    case,
                    sources::Case::IncrementHighInserted | sources::Case::IncrementZeroInserted
                )
            );
            views += 1;
        }
    }
    assert_eq!(views, 14);
}

#[test]
#[ignore = "explicit create-new original/literal source export; no generation"]
fn export_increment_and_random_default_sources() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_NORMAL_AUTONUMBER_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let mut manifest = String::new();
    for case in sources::CASES {
        for header in [5_u32, 6] {
            let mut bytes = sources::source(case).bytes;
            bytes[20..24].copy_from_slice(&header.to_le_bytes());
            let name = format!("h{header}-{}.accdb", case.id());
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join(&name))
                .unwrap();
            file.write_all(&bytes).unwrap();
            let digest = Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            manifest.push_str(&format!("{name}\t{}\t{digest}\n", bytes.len()));
        }
    }
    fs::write(directory.join("manifest.tsv"), manifest).unwrap();
}
