//! Read-only qualification of explicitly selected, independently authored images.
use super::gates;
use crate::{
    FileFormat, NativeImage, check_table,
    format::{
        allocation::{self, MapRole, RepresentedMap},
        reader::Reader,
    },
    inspect_relationships, read_table,
    table::mutation::variable_split::metadata::Metadata,
};
use std::{fs, fs::File, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
#[ignore = "Root-only read qualification of six frozen authored v2 sources; no writer or preview"]
fn qualify_canonical_creation_v2_originals_without_mutation() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_CANONICAL_CREATION_V2_SOURCE_DIR")
            .expect("explicit authored source directory is required"),
    );
    assert!(directory.is_absolute());

    for (name, table, columns, expected_rows, candidate_pages) in [
        ("empty", "Items", vec!["Id", "Score"], Vec::new(), vec![8]),
        (
            "one",
            "Items",
            vec!["Id", "Score"],
            vec![vec![Value::Int32(1), Value::Int32(10)]],
            vec![8],
        ),
        (
            "extrema",
            "Items",
            vec!["Id", "Score"],
            vec![
                vec![Value::Int32(1), Value::Int32(i32::MIN)],
                vec![Value::Int32(2), Value::Int32(i32::MAX)],
            ],
            vec![8],
        ),
        (
            "boundary",
            "Items",
            vec!["Id", "Score"],
            (1..=64)
                .map(|id| vec![Value::Int32(id), Value::Int32(id * 10)])
                .collect(),
            vec![8],
        ),
        (
            "named",
            "People Stats",
            vec!["Person Id", "Score Δ"],
            vec![vec![Value::Int32(7), Value::Int32(42)]],
            vec![8],
        ),
        (
            "wide",
            "Wide Rows",
            vec![
                "C01", "C02", "C03", "C04", "C05", "C06", "C07", "C08", "C09", "C10", "C11", "C12",
                "C13", "C14", "C15", "C16",
            ],
            (1..=58)
                .map(|row| {
                    (1..=16)
                        .map(|column| Value::Int32(row * 100 + column))
                        .collect()
                })
                .collect(),
            Vec::new(),
        ),
    ] {
        let path = directory.join(format!("canonical-v2-h5-{name}.accdb"));
        assert_eq!(fs::metadata(&path).unwrap().len(), 36_864);
        let before = fs::read(&path).unwrap();
        let mut input = File::open(&path).unwrap();
        let image = NativeImage::capture(&mut input, SCOPE, &mut budget(), &|| false).unwrap();
        assert_eq!(image.as_bytes(), before);
        assert_eq!(image.catalog().header().format(), FileFormat::Ace2016);
        assert_eq!(image.catalog().header().page_count(), 9);
        assert_eq!(image.catalog().catalog().objects().len(), 8);
        assert_eq!(image.catalog().origins().len(), 8);

        let objects = image.catalog().catalog().objects();
        let names: Vec<_> = objects
            .iter()
            .map(|object| object.name().as_str())
            .collect();
        assert_eq!(
            names,
            [
                "DatabaseRoot",
                "Tables",
                "Databases",
                "Relationships",
                "MSysObjects",
                "MSysRelationships",
                "MSysACEs",
                table,
            ]
        );
        for (origin, expected_id) in image.catalog().origins().iter().zip([
            0x0f00_0000,
            0x0f00_0001,
            0x0f00_0002,
            0x0f00_0003,
            2,
            5,
            6,
            7,
        ]) {
            assert_eq!(origin.native_id(), expected_id);
        }

        let mut resources = budget();
        let inspection = crate::inspect_table_indexes(
            &mut image.reader(),
            SCOPE,
            table,
            &mut resources,
            &|| false,
        )
        .unwrap();
        let schema = inspection.schema();
        assert_eq!(schema.definition_page(), 7);
        assert_eq!(schema.declared_rows() as usize, expected_rows.len());
        assert_eq!(schema.columns().len(), columns.len());
        assert_eq!(schema.layout.max_columns, columns.len());
        assert_eq!(schema.layout.variable_slots, 0);
        assert_eq!(schema.layout.fixed_end, 2 + columns.len() * 4);
        assert!(inspection.indexes().is_empty());
        for (position, (column, name)) in schema
            .columns()
            .iter()
            .zip(columns.iter().copied())
            .enumerate()
        {
            assert_eq!(column.name(), name);
            assert_eq!(column.number(), position);
            assert_eq!(column.scalar_type(), Some(ScalarType::Int32));
            assert_eq!(
                (
                    column.native_type(),
                    column.storage_length(),
                    column.flags(),
                    column.extended_flags()
                ),
                (4, 4, 3, 0)
            );
        }
        let rows =
            read_table(&mut image.reader(), SCOPE, table, &mut resources, &|| false).unwrap();
        assert_eq!(rows.rows(), expected_rows);
        let checked =
            check_table(&mut image.reader(), SCOPE, table, &mut resources, &|| false).unwrap();
        assert_eq!(checked.rows(), expected_rows.len());
        assert!(checked.indexes().is_empty());

        for (table, count, columns) in [
            ("MSysObjects", 8, 15),
            ("MSysRelationships", 0, 8),
            ("MSysACEs", 0, 4),
        ] {
            let snapshot =
                read_table(&mut image.reader(), SCOPE, table, &mut resources, &|| false).unwrap();
            assert_eq!(snapshot.rows().len(), count);
            assert_eq!(snapshot.schema().columns().len(), columns);
        }
        assert!(
            inspect_relationships(&mut image.reader(), SCOPE, &mut resources, &|| false)
                .unwrap()
                .is_empty()
        );

        {
            let mut bytes = image.reader();
            let mut reader = Reader {
                source: &mut bytes,
                header: image.catalog().header(),
                budget: &mut resources,
                cancelled: &|| false,
                reads: 0,
            };
            let global = RepresentedMap::read(
                &mut reader,
                allocation::GLOBAL_FREE_REFERENCE,
                MapRole::GlobalFree,
            )
            .unwrap();
            global
                .require_allocated_storage(&mut reader, &global)
                .unwrap();
            for page in 0..9 {
                global.require_allocated(&mut reader, page).unwrap();
            }
            for (slot, expected) in [
                (0, vec![4]),
                (1, vec![]),
                (2, vec![]),
                (3, vec![8]),
                (4, vec![4]),
                (5, vec![]),
                (6, vec![]),
                (7, candidate_pages),
                (8, vec![]),
                (9, vec![]),
                (10, vec![]),
                (11, vec![]),
                (12, vec![]),
                (13, vec![]),
                (14, vec![]),
                (15, vec![]),
            ] {
                let reference = 3 * 256 + slot;
                let role = match slot {
                    0..=3 | 8 | 10 | 12 | 14 => MapRole::Owned,
                    4..=7 => MapRole::TableFreeSpace,
                    _ => MapRole::LongValueCandidate,
                };
                let map = RepresentedMap::read(&mut reader, reference, role).unwrap();
                map.require_allocated_storage(&mut reader, &global).unwrap();
                assert_eq!(allocation::pages(&mut reader, reference).unwrap(), expected);
            }
        }

        // Ordinary structural/property/payload/constraint admission is the
        // appropriate existing profile for this all-scalar table. This helper
        // invokes no mutation planner and does not grant normal ACE saves.
        let structural = gates::before(&image, table, &mut resources, &|| false).unwrap();
        assert_eq!(structural.rows(), expected_rows.len());
        let metadata = Metadata::capture_for_insert(
            &image,
            structural.schema(),
            structural.indexes(),
            &mut resources,
            &|| false,
        )
        .unwrap();
        metadata
            .require_source(&image, structural.schema().definition_page())
            .unwrap();

        if name == "wide" {
            let page = &image.as_bytes()[8 * 4096..9 * 4096];
            assert_eq!(u16::from_le_bytes(page[2..4].try_into().unwrap()), 22);
            assert_eq!(u16::from_le_bytes(page[12..14].try_into().unwrap()), 58);
        }

        // Independently assert the complete constructive declaration fields;
        // reader acceptance alone did not establish these missing v1 fields.
        let raw = image.as_bytes();
        for (page, variable_slots) in [
            (2, &[0_u16, 0, 0, 1, 1, 5, 6, 7, 8, 1, 2, 3, 4, 5, 5][..]),
            (5, &[0, 1, 1, 1, 1, 2, 3, 4][..]),
            (6, &[0, 0, 0, 0][..]),
            (7, &[0_u16; 16][..columns.len()]),
        ] {
            for (field, variable_slot) in variable_slots.iter().enumerate() {
                let descriptor = page * 4096 + 63 + field * 25;
                assert_eq!(
                    &raw[descriptor + 7..descriptor + 9],
                    &variable_slot.to_le_bytes()
                );
                if raw[descriptor] == 1 {
                    // Boolean is physically stored in its mask bit. The
                    // constructive fixed offset is zero, not a payload slot.
                    assert_eq!(&raw[descriptor + 21..descriptor + 23], &[0, 0]);
                }
                assert_eq!(
                    &raw[descriptor + 1..descriptor + 5],
                    &1625_u32.to_le_bytes()
                );
                assert_eq!(
                    &raw[descriptor + 9..descriptor + 11],
                    &(field as u16).to_le_bytes()
                );
                assert_eq!(raw[descriptor + 16], 0);
                assert_eq!(&raw[descriptor + 17..descriptor + 21], &[0, 0, 0, 0]);
                match raw[descriptor] {
                    9..=11 => {
                        assert_eq!(raw[descriptor + 15], 2);
                        assert_eq!(&raw[descriptor + 21..descriptor + 23], &[0, 0]);
                    }
                    _ => assert_eq!(raw[descriptor + 15], 3),
                }
                if raw[descriptor] == 10 {
                    assert_eq!(&raw[descriptor + 11..descriptor + 15], &[9, 4, 0, 1]);
                } else {
                    assert_eq!(&raw[descriptor + 11..descriptor + 15], &[0, 0, 0, 0]);
                }
            }
        }
        let catalog_definition = &raw[2 * 4096..3 * 4096];
        let length = u32::from_le_bytes(catalog_definition[8..12].try_into().unwrap()) as usize;
        let mut position = 63 + 15 * 25;
        for _ in 0..15 {
            let bytes = u16::from_le_bytes(
                catalog_definition[position..position + 2]
                    .try_into()
                    .unwrap(),
            ) as usize;
            position += 2 + bytes;
        }
        // Four actual long-value declarations, each with separately owned
        // and candidate map records, precede the complete FFFF terminator.
        for (field, owned, candidate) in
            [(5_u16, 8_u8, 9_u8), (6, 10, 11), (7, 12, 13), (8, 14, 15)]
        {
            assert_eq!(
                &catalog_definition[position..position + 2],
                &field.to_le_bytes()
            );
            assert_eq!(
                &catalog_definition[position + 2..position + 6],
                &[owned, 3, 0, 0]
            );
            assert_eq!(
                &catalog_definition[position + 6..position + 10],
                &[candidate, 3, 0, 0]
            );
            position += 10;
        }
        assert_eq!(position + 2, length);
        assert_eq!(&catalog_definition[position..length], &[0xff, 0xff]);
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(image.as_bytes(), before);
    }
}

#[cfg(feature = "journal")]
mod rust_fixture;
