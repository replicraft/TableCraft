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
#[ignore = "Root-only read qualification of three frozen authored sources; no writer or preview"]
fn qualify_canonical_creation_originals_without_mutation() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_CANONICAL_CREATION_SOURCE_DIR")
            .expect("explicit authored source directory is required"),
    );
    assert!(directory.is_absolute());

    for (name, expected_rows) in [
        ("empty", Vec::new()),
        ("one", vec![vec![Value::Int32(1), Value::Int32(10)]]),
        (
            "extrema",
            vec![
                vec![Value::Int32(1), Value::Int32(i32::MIN)],
                vec![Value::Int32(2), Value::Int32(i32::MAX)],
            ],
        ),
    ] {
        let path = directory.join(format!("canonical-h5-{name}.accdb"));
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
                "Items",
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
            "Items",
            &mut resources,
            &|| false,
        )
        .unwrap();
        let schema = inspection.schema();
        assert_eq!(schema.definition_page(), 7);
        assert_eq!(schema.declared_rows() as usize, expected_rows.len());
        assert_eq!(schema.columns().len(), 2);
        assert_eq!(schema.layout.max_columns, 2);
        assert_eq!(schema.layout.variable_slots, 0);
        assert_eq!(schema.layout.fixed_end, 10);
        assert!(inspection.indexes().is_empty());
        for (position, (column, name)) in schema.columns().iter().zip(["Id", "Score"]).enumerate() {
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
        let rows = read_table(&mut image.reader(), SCOPE, "Items", &mut resources, &|| {
            false
        })
        .unwrap();
        assert_eq!(rows.rows(), expected_rows);
        let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut resources, &|| {
            false
        })
        .unwrap();
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
                (7, vec![8]),
            ] {
                let reference = 3 * 256 + slot;
                let role = if slot < 4 {
                    MapRole::Owned
                } else {
                    MapRole::TableFreeSpace
                };
                let map = RepresentedMap::read(&mut reader, reference, role).unwrap();
                map.require_allocated_storage(&mut reader, &global).unwrap();
                assert_eq!(allocation::pages(&mut reader, reference).unwrap(), expected);
            }
        }

        // Ordinary structural/property/payload/constraint admission is the
        // appropriate existing profile for this all-scalar table. This helper
        // invokes no mutation planner and does not grant normal ACE saves.
        let structural = gates::before(&image, "Items", &mut resources, &|| false).unwrap();
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

        // Record constructive omissions independently of reader admission:
        // current declaration parsing does not require descriptor magic or
        // columnId, and an immediate FFFF is not proof that every LVAL
        // declaration has a usage-map pair. These facts describe this frozen
        // first hypothesis, rather than asserting a fabricated parse error.
        let raw = image.as_bytes();
        for (page, fields) in [(2, 15), (5, 8), (6, 4), (7, 2)] {
            for field in 0..fields {
                let descriptor = page * 4096 + 63 + field * 25;
                assert_eq!(&raw[descriptor + 1..descriptor + 5], &[0, 0, 0, 0]);
                assert_eq!(&raw[descriptor + 9..descriptor + 11], &[0, 0]);
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
        // All four declared catalog LVAL carriers currently lack constructive
        // ten-byte pairs. They are NULL in every row, but that does not supply
        // the missing serializer declarations or complete creation authority.
        assert_eq!(position + 2, length);
        assert_eq!(&catalog_definition[position..length], &[0xff, 0xff]);
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(image.as_bytes(), before);
    }
}
