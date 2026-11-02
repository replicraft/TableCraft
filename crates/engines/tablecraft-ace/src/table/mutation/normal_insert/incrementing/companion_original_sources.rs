//! Existing SDK originals only: no models, generated rows, planner or creator.
use crate::table::mutation::{autonumber_counter, record_insertion};
use crate::{
    IndexCheckStatus, NativeImage, check_table, check_table_constraints,
    inspect_table_index_counter, inspect_table_properties, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(256);
const CASES: [(u32, usize, usize, usize, usize, &str); 2] = [
    (
        5,
        446464,
        94,
        97,
        96,
        "c6dc5d36f35335ea2d9cafbc5cf6500dc135ea41a4d3f60d9241ac9e55714d12",
    ),
    (
        6,
        520192,
        111,
        115,
        113,
        "c6aae0208a2dfbd0b77f8867f49700718880ee761c7881df6ec5789843a8b456",
    ),
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires pinned existing SDK Numbered originals; read-only Source before any new AFTER model"]
fn existing_numbered_originals_bind_middle_generator_aliases_deleted_high_and_actual_counter_source()
 {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_INCREMENT_COMPANION_ORIGINAL_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut originals = Vec::new();
    for (header, length, definition, data, root, wanted) in CASES {
        let raw = fs::read(directory.join(format!("h{header}-default-original.accdb"))).unwrap();
        assert_eq!(raw.len(), length);
        assert_eq!(
            Sha256::digest(&raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
        originals.push((header, definition, data, root, raw));
    }
    for (header, definition, data, root, raw) in originals {
        assert_eq!(&raw[20..24], header.to_le_bytes());
        let image =
            NativeImage::capture(&mut Cursor::new(&raw), SCOPE, &mut budget(), &|| false).unwrap();
        let expected = [
            vec![Value::Int32(101), Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(102), Value::Int32(2), Value::Int32(20)],
        ];
        let table = read_table(
            &mut image.reader(),
            SCOPE,
            "Numbered",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(table.rows(), expected);
        assert_eq!(table.schema().definition_page() as usize, definition);
        assert_eq!(
            table
                .schema()
                .columns()
                .iter()
                .map(|column| (
                    column.name(),
                    column.number(),
                    column.native_type(),
                    column.flags(),
                    column.storage_length(),
                    column.extended_flags()
                ))
                .collect::<Vec<_>>(),
            [
                ("Earlier", 0, 4, 3, 4, 0),
                ("Id", 1, 4, 7, 4, 0),
                ("Score", 2, 4, 3, 4, 0)
            ]
        );
        let properties = inspect_table_properties(
            &mut image.reader(),
            SCOPE,
            "Numbered",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(properties.raw_bytes().is_empty());
        assert!(properties.groups().is_empty());
        let checked = check_table(
            &mut image.reader(),
            SCOPE,
            "Numbered",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), 2);
        assert_eq!(checked.indexes().len(), 2);
        let mut physical = None;
        for index in checked.indexes() {
            assert_eq!(index.status(), IndexCheckStatus::Checked);
            assert_eq!(index.index().root_page() as usize, root);
            let [column] = index.index().columns() else {
                panic!("single generated LONG primary component");
            };
            assert_eq!(column.number(), 1);
            assert!(!column.descending());
            let number = index.index().data_number();
            assert_eq!(*physical.get_or_insert(number), number);
        }
        for alias in ["PrimaryID", "PrimaryAlias"] {
            assert_eq!(
                read_table_index(
                    &mut image.reader(),
                    SCOPE,
                    "Numbered",
                    alias,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                expected
            );
            assert_eq!(
                inspect_table_index_counter(
                    &mut image.reader(),
                    SCOPE,
                    "Numbered",
                    alias,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .value(),
                3
            );
        }
        assert!(
            check_table_constraints(
                &mut image.reader(),
                SCOPE,
                "Numbered",
                &mut budget(),
                &|| false
            )
            .unwrap()
            .relationships()
            .is_empty()
        );
        let source =
            super::super::capture_source(&image, "Numbered", &mut budget(), &|| false).unwrap();
        assert_eq!(source.table.rows(), expected);
        assert_eq!(source.records.len(), 2);
        let schema = source.table.schema();
        assert!(super::mode::selected(schema, &mut budget(), &|| false).unwrap());
        // Neutral exact declaration/LAST facts, never generation arithmetic.
        let counter =
            autonumber_counter::capture(&image, schema, 1, &mut budget(), &|| false).unwrap();
        assert_eq!(counter.column, 1);
        assert_eq!(counter.last, 3);
        assert_eq!(counter.before, 3_i32.to_le_bytes());
        assert_eq!(counter.offset, definition * 4096 + 20);
        assert!(matches!(
            super::mode::inspect(&image, schema, &mut budget(), &|| false).unwrap(),
            super::mode::Mode::Increment
        ));
        assert!(
            record_insertion::layout::incrementing_variables(schema, 1, &mut budget(), &|| false)
                .unwrap()
                .is_empty()
        );
        let shape = record_insertion::proof::layout::capture_incrementing(
            schema,
            1,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(shape.fixed_end, 14);
        assert_eq!(shape.mask_bytes, 1);
        assert!(shape.variables.is_empty());
        let base = data * 4096;
        assert_eq!(&raw[base + 12..base + 14], &3_u16.to_le_bytes());
        assert_eq!(&raw[base + 2..base + 4], &4031_u16.to_le_bytes());
        for (slot, word) in [0x0ff1_u16, 0x0fe2, 0xcfd3].into_iter().enumerate() {
            assert_eq!(
                &raw[base + 14 + 2 * slot..base + 16 + 2 * slot],
                &word.to_le_bytes()
            );
        }
        for (ordinal, record) in source.records.iter().enumerate() {
            assert_eq!(record.reference, ((data as u32) << 8) | ordinal as u32);
            assert_eq!(record.offset, base + 4081 - ordinal * 15);
            let mut expected_record = Vec::from([3, 0]);
            for value in [
                101 + ordinal as i32,
                1 + ordinal as i32,
                10 * (ordinal as i32 + 1),
            ] {
                expected_record.extend_from_slice(&value.to_le_bytes());
            }
            expected_record.push(7);
            assert_eq!(record.data, expected_record);
        }
        assert_eq!(
            &raw[base + 4051..base + 4066],
            &[3, 0, 103, 0, 0, 0, 3, 0, 0, 0, 30, 0, 0, 0, 7]
        );
        assert_eq!(image.as_bytes(), raw);
    }
}
