//! Unchanged unindexed and indexed GUID originals, before normal UPDATE widening.
use crate::{
    GuidIndexSelection, IndexCheckStatus, NativeImage, check_table, inspect_table_index_counter,
    inspect_table_schema, read_table, read_table_guid_index, read_table_index,
    table::mutation::{no_required_programs::NoRequiredTablePrograms, normal_ordinary},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_creation as fixed, canonical_ordinary_wide_insert as wide,
    genuine_indexed_guid_update::raw::{
        ORIGINAL_PINS, PAGE, entries, geometry, guid, indexed_records, indexed_rows, leaf, record,
    },
};

const SCOPE: CatalogScope = CatalogScope::new(295);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "six unchanged GUID originals; read/Source observations only, no UPDATE"]
fn existing_guid_originals_keep_data_keys_and_observe_source_admission() {
    let wide_dir =
        PathBuf::from(std::env::var_os("TABLECRAFT_GUID_UPDATE_WIDE_SOURCE_DIR").unwrap());
    let indexed_dir =
        PathBuf::from(std::env::var_os("TABLECRAFT_GUID_UPDATE_INDEX_SOURCE_DIR").unwrap());
    assert!(wide_dir.is_absolute() && indexed_dir.is_absolute());
    let images: Vec<_> = ORIGINAL_PINS
        .iter()
        .map(|&(header, direction, name, pin)| {
            let raw = fs::read(if direction.is_some() {
                indexed_dir.join(name)
            } else {
                wide_dir.join(name)
            })
            .unwrap();
            let digest = Sha256::digest(&raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(digest, pin);
            assert_eq!(&raw[20..24], &u32::from(header).to_le_bytes());
            (header, direction, raw)
        })
        .collect();

    for (header, direction, raw) in &images {
        let image =
            NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
        let schema =
            inspect_table_schema(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
        let expected = if direction.is_some() {
            indexed_rows()
        } else {
            let source = wide::source(wide::Case::FixedExact);
            let mut rows = source.rows_before;
            rows.push(source.values);
            rows
        };
        let references = if let Some(descending) = direction {
            let (definition, data, roots) = geometry(*header);
            assert_eq!(schema.definition_page() as usize, definition);
            assert_eq!(schema.columns().len(), 3);
            for offset in [67, 79] {
                let start = definition * PAGE + offset;
                assert_eq!(&raw[start..start + 4], &158_u32.to_le_bytes());
            }

            for (position, field) in schema.columns().iter().enumerate() {
                assert_eq!(field.number(), position);
                assert_eq!(field.name(), ["ID", "Group", "Score"][position]);
                assert_eq!(field.native_type(), [4, 15, 4][position]);
                assert_eq!(field.storage_length(), [4, 16, 4][position]);
                assert!(field.fixed());
                assert_eq!((field.flags(), field.extended_flags()), (3, 0));
            }
            let references = indexed_records(raw, *header);
            let keys = [
                entries(&expected, &references, false, false),
                entries(&expected, &references, true, *descending),
            ];
            for (root, wire) in roots.iter().zip(&keys) {
                leaf(raw, definition, *root, wire);
            }

            let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(checked.rows(), 158);
            assert_eq!(checked.indexes().len(), 4);
            for checked in checked.indexes() {
                assert_eq!(checked.status(), IndexCheckStatus::Checked);
                let index = checked.index();
                let secondary = match index.name() {
                    "PrimaryID" | "PrimaryAlias" => false,
                    "GroupOrder" | "GroupAlias" => true,
                    _ => panic!("outside four frozen aliases"),
                };
                assert_eq!(index.root_page() as usize, roots[usize::from(secondary)]);
                assert_eq!(index.data_number(), u32::from(secondary));
                assert_eq!(index.columns().len(), 1);
                assert_eq!(index.columns()[0].number(), usize::from(secondary));
                assert_eq!(index.columns()[0].descending(), secondary && *descending);
                let constraints = index.constraints().unwrap();
                assert_eq!(constraints.primary_key(), index.name() == "PrimaryID");
                assert_eq!(constraints.required(), !secondary);
                assert_eq!(constraints.unique(), !secondary);
                assert!(!constraints.ignore_nulls());
                let wire = &keys[usize::from(secondary)];
                let observed = if secondary {
                    read_table_guid_index(
                        &mut image.reader(),
                        SCOPE,
                        "Items",
                        index.name(),
                        GuidIndexSelection::All,
                        &mut budget(),
                        &|| false,
                    )
                } else {
                    read_table_index(
                        &mut image.reader(),
                        SCOPE,
                        "Items",
                        index.name(),
                        &mut budget(),
                        &|| false,
                    )
                }
                .unwrap();
                assert_eq!(
                    observed.rows(),
                    wire.iter()
                        .map(|(_, slot)| expected[*slot].clone())
                        .collect::<Vec<_>>()
                );
                let counter = inspect_table_index_counter(
                    &mut image.reader(),
                    SCOPE,
                    "Items",
                    index.name(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(counter.value(), 158);
            }
            let nulls = read_table_guid_index(
                &mut image.reader(),
                SCOPE,
                "Items",
                "GroupOrder",
                GuidIndexSelection::Null,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(nulls.rows(), &[expected[157].clone()]);
            let last = read_table_guid_index(
                &mut image.reader(),
                SCOPE,
                "Items",
                "GroupOrder",
                GuidIndexSelection::Equal(guid(156)),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(last.rows(), &[expected[156].clone()]);
            assert_eq!(data, if *header == 5 { [93, 95] } else { [111, 113] });
            references
        } else {
            assert_eq!(raw.len(), 36864);
            assert_eq!(schema.definition_page(), 7);
            assert_eq!(schema.columns().len(), 10);
            for (number, (field, literal)) in
                schema.columns().iter().zip(&fixed::COLUMNS).enumerate()
            {
                assert_eq!(field.number(), number);
                assert_eq!(field.name(), literal.name);
                assert_eq!(field.native_type(), literal.native_type);
                assert_eq!(field.storage_length(), usize::from(literal.width));
                assert!(field.fixed());
                assert_eq!((field.flags(), field.extended_flags()), (3, 0));
            }
            let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(checked.rows(), 3);
            assert!(checked.indexes().is_empty());
            let selected = record(raw, 8, 2, 63);
            assert_eq!(
                &selected[45..61],
                &[
                    0x33, 0x22, 0x11, 0, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                    0xdd, 0xee, 0xff
                ]
            );
            assert_eq!(&selected[61..63], &[0xff, 3]);
            (0..3)
                .map(|slot| ((8 << 8) | slot as u32, record(raw, 8, slot, 63).to_vec()))
                .collect()
        };
        let read = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(read.rows(), expected);
        match NoRequiredTablePrograms::capture_updates(&image, &schema, &mut budget(), &|| false) {
            Ok(_) => eprintln!(
                "GUID_NO_REQUIRED header={header} indexed={} descending={direction:?} ADMITTED",
                direction.is_some()
            ),
            Err(error) => eprintln!(
                "GUID_NO_REQUIRED header={header} indexed={} descending={direction:?} REFUSED {error:?}",
                direction.is_some()
            ),
        }
        match normal_ordinary::capture(&image, "Items", &mut budget(), &|| false) {
            Ok(source) => {
                assert_eq!(source.table.rows(), expected);
                assert_eq!(source.records.len(), references.len());
                for (record, (reference, bytes)) in source.records.iter().zip(&references) {
                    assert_eq!(record.reference, *reference);
                    assert_eq!(&record.data, bytes);
                }
                eprintln!(
                    "GUID_CANONICAL_SOURCE header={header} indexed={} descending={direction:?} ADMITTED",
                    direction.is_some()
                );
            }
            Err(error) => eprintln!(
                "GUID_CANONICAL_SOURCE header={header} indexed={} descending={direction:?} REFUSED {error:?}",
                direction.is_some()
            ),
        }
        assert_eq!(image.as_bytes(), raw);
    }
    assert_eq!(images.len(), 6);
}

#[cfg(test)]
mod score_plan;

#[cfg(test)]
mod indexed_updates;
