//! Direct read-only preservation admission of eight frozen forwarded expectations.
use super::super::super::source;
use crate::{
    FileFormat, NativeImage, read_attachment_column, read_stored_int32_calculation,
    read_table_projection,
};
use std::{fs::File, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(1);
const MAX_IMAGE_BYTES: u64 = 2 * 1024 * 1024;
const PAGE_BYTES: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
#[ignore = "Explicit frozen expected directory required; read-only source admission, no writer calls"]
fn qualify_genuine_preserved_text_growth_expectations_before_producer() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_PRESERVED_TEXT_GROWTH_EXPECTED_DIR")
            .expect("explicit frozen expected directory is required"),
    );
    assert!(directory.is_absolute());
    let mut admitted = 0;
    for (header, format, parent) in [
        (5, FileFormat::Ace2016, 100_usize),
        (6, FileFormat::Ace2019, 118_usize),
    ] {
        for name in ["present", "null", "batch", "batch-reverse"] {
            let path = directory.join(format!("h{header}-{name}.accdb"));
            let mut input = File::open(&path).unwrap();
            assert!(input.metadata().unwrap().len() <= MAX_IMAGE_BYTES);
            let image = NativeImage::capture(&mut input, SCOPE, &mut budget(), &|| false).unwrap();
            assert_eq!(image.catalog().header().format(), format);
            // The actual complete Source owner checks properties, catalog,
            // GLOBAL/maps/index roles, relationships, LVALs and attachments.
            // No preview, stage, mutation proof or no-op writer is called.
            let qualified =
                source::capture(&image, "PreservedRows", &mut budget(), &|| false).unwrap();
            assert_eq!(qualified.table.schema().columns().len(), 8);
            let target = &qualified.table.schema().columns()[1];
            assert_eq!(target.name(), "Short_Text");
            assert!(target.is_ordinary_short_text());
            assert_eq!(qualified.dependencies, [2]);
            assert!(
                qualified
                    .indexes
                    .indexes()
                    .iter()
                    .all(|index| index.columns().iter().all(|column| column.number() != 1))
            );
            assert_eq!(qualified.records.len(), 3);
            assert_eq!(
                qualified
                    .records
                    .iter()
                    .map(|record| record.reference)
                    .collect::<Vec<_>>(),
                [
                    (parent as u32) << 8,
                    ((parent as u32) << 8) | 1,
                    ((parent as u32) << 8) | 2
                ]
            );
            let snow = Value::Text("雪雪雪雪雪雪".into());
            let (first, second, count, free) = match name {
                "present" => (snow, Value::Null, 4, 3688),
                "null" => (Value::Text("alpha".into()), snow, 4, 3736),
                "batch" | "batch-reverse" => (snow, Value::Text("beta".into()), 5, 3614),
                _ => unreachable!(),
            };
            let rows = read_table_projection(
                &mut image.reader(),
                SCOPE,
                "PreservedRows",
                &["ID", "Short_Text", "Base", "Flag"],
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                rows.rows(),
                [
                    vec![
                        Value::Int32(1),
                        first,
                        Value::Int32(7),
                        Value::Boolean(false)
                    ],
                    vec![
                        Value::Int32(2),
                        second,
                        Value::Int32(11),
                        Value::Boolean(true)
                    ],
                    vec![
                        Value::Int32(3),
                        Value::Text("omega".into()),
                        Value::Int32(13),
                        Value::Boolean(false)
                    ],
                ]
            );
            let data = &image.as_bytes()[parent * PAGE_BYTES..(parent + 1) * PAGE_BYTES];
            assert_eq!(u16::from_le_bytes(data[12..14].try_into().unwrap()), count);
            assert_eq!(u16::from_le_bytes(data[2..4].try_into().unwrap()), free);
            for ordinal in 0..3 {
                let selected = if name == "present" {
                    ordinal == 0
                } else if name == "null" {
                    ordinal == 1
                } else {
                    ordinal < 2
                };
                let at = 14 + ordinal * 2;
                let word = u16::from_le_bytes(data[at..at + 2].try_into().unwrap());
                assert_eq!(word & 0x4000 != 0, selected);
                if selected {
                    let root = usize::from(word & 0x1fff);
                    let slot = if name == "batch" || name == "batch-reverse" {
                        3 + ordinal
                    } else {
                        3
                    };
                    let pointer = u32::from_le_bytes(data[root..root + 4].try_into().unwrap());
                    assert_eq!(pointer, ((parent as u32) << 8) | slot as u32);
                    let hidden = 14 + slot * 2;
                    let hidden_word =
                        u16::from_le_bytes(data[hidden..hidden + 2].try_into().unwrap());
                    assert_ne!(hidden_word & 0x8000, 0);
                    assert_eq!(
                        qualified.records[ordinal].offset,
                        parent * PAGE_BYTES + usize::from(hidden_word & 0x1fff)
                    );
                }
            }
            let cached = read_stored_int32_calculation(
                &mut image.reader(),
                SCOPE,
                "PreservedRows",
                "Cached",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(cached.values(), &[Some(999), Some(-333), Some(0)]);
            let attachments = read_attachment_column(
                &mut image.reader(),
                SCOPE,
                "PreservedRows",
                "Files",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                attachments
                    .links()
                    .parents()
                    .iter()
                    .map(|parent| parent.child_count())
                    .collect::<Vec<_>>(),
                [2, 1, 0]
            );
            assert_eq!(std::fs::read(path).unwrap(), image.as_bytes());
            admitted += 1;
        }
    }
    assert_eq!(admitted, 8);
}
