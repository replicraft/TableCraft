//! Existing genuine originals and independent fitting outcomes, before normal saves.
use crate::{
    IndexCheckStatus, Int32IndexSelection, NativeImage, check_table, check_table_constraints,
    inspect_table_index_counter, inspect_table_schema, read_table, read_table_index,
    read_table_index_range,
    table::mutation::{no_required_programs::NoRequiredTablePrograms, normal_ordinary},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, ops::Bound, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(261);
const ALIASES: [&str; 4] = ["PrimaryID", "PrimaryAlias", "ValueOrder", "ValueAlias"];
const PINS: &[(u8, &str, usize, &str)] = &[
    (
        5,
        "V2016-source.accdb",
        532480,
        "74f6ac1c99b0244a69427afb379112c7321690b9ba628417b1d9411bbb232258",
    ),
    (
        6,
        "V2019-source.accdb",
        602112,
        "86e518e9d7c9d2b77d72c0457d369748924045c05103aaf6be472f7eecd8bd1a",
    ),
    (
        5,
        "V2016-single-null-new-expected.accdb",
        532480,
        "e60cc6d4b66e7b3fb6ee4e6a80295215d1d332e332524d12a43545e02f7b7b2f",
    ),
    (
        5,
        "V2016-single-null-existing-expected.accdb",
        532480,
        "cb44abb8dbddc974d27380621a8836170d3871d4d405bb97a511902af7cc6cfb",
    ),
    (
        5,
        "V2016-single-present-null-expected.accdb",
        532480,
        "47b11f15cc98000e675293a37726387dd3862a34781ea7ee139dceba07e22df4",
    ),
    (
        6,
        "V2019-single-null-new-expected.accdb",
        602112,
        "21a9711a6fa39094c99c7191ad32a4dfd6a74d97f40515d0eefc32ff75513974",
    ),
    (
        6,
        "V2019-single-null-existing-expected.accdb",
        602112,
        "8f44acb6268c3955b3c0d6fba5ba018ea2a6ec6872605bdcd5ced2b70babd9e4",
    ),
    (
        6,
        "V2019-single-present-null-expected.accdb",
        602112,
        "9f9626dd04100e3b0829b55db458c9c7f877db219efe10f5b69180a8f9a29969",
    ),
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn rows(table: &str, recipe: &str) -> Vec<Vec<Value>> {
    let cells: Vec<[Option<i32>; 4]> = match table {
        "Single" => vec![
            [Some(1), Some(10), Some(100), Some(1001)],
            [Some(2), Some(10), Some(200), Some(1002)],
            [Some(3), None, Some(300), Some(1003)],
            [Some(4), Some(20), None, Some(1004)],
        ],
        "UniqueSingle" => vec![
            [Some(1), Some(10), Some(100), Some(1001)],
            [Some(2), Some(20), Some(200), Some(1002)],
            [Some(3), None, Some(300), Some(1003)],
            [Some(4), None, None, Some(1004)],
        ],
        "RequiredSingle" => vec![
            [Some(1), Some(10), Some(100), Some(1001)],
            [Some(2), Some(20), None, Some(1002)],
        ],
        _ => panic!("outside the held single-key corpus"),
    };
    let mut result: Vec<Vec<Value>> = cells
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|cell| cell.map(Value::Int32).unwrap_or(Value::Null))
                .collect()
        })
        .collect();

    match recipe {
        "before" => {}
        "single-null-new" => result[2][1] = Value::Int32(30),
        "single-null-existing" => result[2][1] = Value::Int32(10),
        "single-present-null" => result[0][1] = Value::Null,
        _ => panic!("outside original request laws"),
    }

    result
}

fn geometry(header: u8, table: &str) -> (usize, usize, [usize; 2]) {
    match (header, table) {
        (5, "Single") => (89, 93, [91, 92]),
        (5, "UniqueSingle") => (101, 105, [103, 104]),
        (5, "RequiredSingle") => (113, 117, [115, 116]),
        (6, "Single") => (106, 110, [108, 109]),
        (6, "UniqueSingle") => (118, 123, [120, 121]),
        (6, "RequiredSingle") => (131, 135, [133, 134]),
        _ => panic!("outside original geometry"),
    }
}

fn wire(rows: &[Vec<Value>], data: usize, column: usize) -> (Vec<usize>, Vec<u8>) {
    let mut entries = Vec::new();
    for (slot, row) in rows.iter().enumerate() {
        if let Value::Int32(value) = row[column] {
            let mut entry = vec![0x7f];
            entry.extend_from_slice(&((value as u32) ^ 0x8000_0000).to_be_bytes());
            entry.extend_from_slice(&(((data as u32) << 8) | slot as u32).to_be_bytes());
            entries.push((entry, slot));
        } else {
            assert_eq!(row[column], Value::Null);
            assert_eq!(column, 1, "primary membership is never omitted");
        }
    }
    entries.sort_unstable();
    let order = entries.iter().map(|(_, slot)| *slot).collect();
    let active = entries.into_iter().flat_map(|(entry, _)| entry).collect();
    (order, active)
}

fn assert_leaf(raw: &[u8], root: usize, definition: usize, active: &[u8]) {
    let page = &raw[root * 4096..(root + 1) * 4096];
    assert_eq!(&page[..2], &[4, 1]);
    assert_eq!(&page[4..8], &(definition as u32).to_le_bytes());
    assert_eq!(&page[8..27], &[0; 19]);
    assert_eq!(&page[2..4], &((3616 - active.len()) as u16).to_le_bytes());

    let mut bitmap = [0_u8; 453];
    for end in (9..=active.len()).step_by(9) {
        bitmap[end / 8] |= 1 << (end % 8);
    }
    assert_eq!(&page[27..480], &bitmap);
    assert_eq!(&page[480..480 + active.len()], active);
}

fn assert_after(before: &[u8], after: &[u8], header: u8, recipe: &str) {
    let (definition, data, roots) = geometry(header, "Single");
    let mut expected = before.to_vec();
    let (slot, value) = match recipe {
        "single-null-new" => (2, Some(30_i32)),
        "single-null-existing" => (2, Some(10_i32)),
        "single-present-null" => (0, None),
        _ => panic!("AFTER requires one existing literal request"),
    };
    let record = data * 4096 + 4096 - (slot + 1) * 19;
    let mask = record + 18;
    if let Some(value) = value {
        expected[record + 6..record + 10].copy_from_slice(&value.to_le_bytes());
        expected[mask] |= 2;
    } else {
        expected[mask] &= !2;
        // The inactive four-byte payload deliberately retains its old value.
        assert_eq!(
            &after[record + 6..record + 10],
            &before[record + 6..record + 10]
        );
    }
    let (_, active) = wire(&rows("Single", recipe), data, 1);
    let root = roots[1] * 4096;
    expected[root + 2..root + 4].copy_from_slice(&((3616 - active.len()) as u16).to_le_bytes());
    expected[root + 27..root + 480].fill(0);
    for end in (9..=active.len()).step_by(9) {
        expected[root + 27 + end / 8] |= 1 << (end % 8);
    }
    expected[root + 480..root + 480 + active.len()].copy_from_slice(&active);
    let history = if recipe == "single-null-new" {
        3_u32
    } else {
        2
    };
    expected[definition * 4096 + 79..definition * 4096 + 83]
        .copy_from_slice(&history.to_le_bytes());
    // Complete complement includes primary, every other table, all map records,
    // free bytes, inactive INDEX tails, catalog and original EOF.
    assert_eq!(expected, after);
}

#[test]
#[ignore = "requires the existing 2026-10-29 IgnoreNULL originals and literal AFTERs"]
fn existing_ignore_null_single_key_images_keep_membership_history_and_native_source_status() {
    let original_dir =
        PathBuf::from(std::env::var_os("TABLECRAFT_IGNORE_NULL_ORIGINAL_DIR").unwrap());
    let expected_dir =
        PathBuf::from(std::env::var_os("TABLECRAFT_IGNORE_NULL_EXPECTED_DIR").unwrap());
    assert!(original_dir.is_absolute() && expected_dir.is_absolute());
    let images: Vec<_> = PINS
        .iter()
        .map(|&(header, name, length, hash)| {
            let original = name.ends_with("-source.accdb");
            let directory = if original {
                &original_dir
            } else {
                &expected_dir
            };
            let raw = fs::read(directory.join(name)).unwrap();
            assert_eq!(raw.len(), length);
            let actual: String = Sha256::digest(&raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(actual, hash);
            (header, name, raw)
        })
        .collect();

    let mut contexts = 0;
    for (header, name, raw) in &images {
        let recipe = if name.ends_with("-source.accdb") {
            "before"
        } else {
            name.strip_prefix(if *header == 5 { "V2016-" } else { "V2019-" })
                .unwrap()
                .strip_suffix("-expected.accdb")
                .unwrap()
        };
        let tables: &[&str] = if recipe == "before" {
            &["Single", "UniqueSingle", "RequiredSingle"]
        } else {
            &["Single"]
        };
        if recipe != "before" {
            let before = &images
                .iter()
                .find(|(h, n, _)| h == header && n.ends_with("-source.accdb"))
                .unwrap()
                .2;
            assert_after(before, raw, *header, recipe);
        }

        for &table in tables {
            let image =
                NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let catalog = inspect_table_schema(
                &mut image.reader(),
                SCOPE,
                "MSysObjects",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(catalog.columns().len(), 17);

            let snapshot =
                read_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
            let expected = rows(table, recipe);
            assert_eq!(snapshot.rows(), expected);
            let schema = snapshot.schema();
            let (definition, data, roots) = geometry(*header, table);
            assert_eq!(schema.definition_page() as usize, definition);
            assert_eq!(schema.columns().len(), 4);
            for (ordinal, field) in schema.columns().iter().enumerate() {
                assert_eq!(field.number(), ordinal);
                assert_eq!(field.name(), ["ID", "Left", "Right", "Marker"][ordinal]);
                assert_eq!(
                    (field.native_type(), field.flags(), field.extended_flags()),
                    (4, 3, 0)
                );
                assert!(field.fixed());
                assert_eq!(field.storage_length(), 4);
            }

            let checked =
                check_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
            assert_eq!(checked.rows(), expected.len());
            assert_eq!(checked.indexes().len(), 4);
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            for selected in checked.indexes() {
                let index = selected.index();
                let physical = usize::try_from(index.data_number()).unwrap();
                assert!(physical < 2);
                assert_eq!(index.root_page() as usize, roots[physical]);
                let flags = index.constraints().unwrap();
                assert_eq!(flags.ignore_nulls(), physical == 1);
                assert_eq!(flags.unique(), physical == 0 || table == "UniqueSingle");
                assert_eq!(flags.required(), physical == 0 || table == "RequiredSingle");
            }

            let constraints =
                check_table_constraints(&mut image.reader(), SCOPE, table, &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert!(constraints.relationships().is_empty());

            for (position, alias) in ALIASES.into_iter().enumerate() {
                let physical = position / 2;
                let (order, active) = wire(&expected, data, physical);
                assert_leaf(raw, roots[physical], definition, &active);
                let observed = if physical == 0 {
                    read_table_index(
                        &mut image.reader(),
                        SCOPE,
                        table,
                        alias,
                        &mut budget(),
                        &|| false,
                    )
                } else {
                    // Omitted NULLs are not a full-table snapshot. Explicitly
                    // select the complete non-NULL domain of this secondary.
                    read_table_index_range(
                        &mut image.reader(),
                        SCOPE,
                        table,
                        alias,
                        Int32IndexSelection::Range {
                            lower: Bound::Unbounded,
                            upper: Bound::Unbounded,
                        },
                        &mut budget(),
                        &|| false,
                    )
                }
                .unwrap();
                assert_eq!(
                    observed.rows(),
                    order
                        .into_iter()
                        .map(|slot| expected[slot].clone())
                        .collect::<Vec<_>>()
                );
                let counter = inspect_table_index_counter(
                    &mut image.reader(),
                    SCOPE,
                    table,
                    alias,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let history = if physical == 0 {
                    expected.len() as u32
                } else if recipe == "single-null-new" {
                    3
                } else {
                    2
                };
                assert_eq!(counter.value(), history);
            }

            // Observe declaration/program admission separately from healthy
            // native reads. A shape refusal does not prove active code exists.
            match NoRequiredTablePrograms::capture_updates(&image, schema, &mut budget(), &|| false)
            {
                Ok(_certificate) => eprintln!(
                    "NO_REQUIRED_IGNORE_NULL_SOURCE header={header} table={table} recipe={recipe} ADMITTED"
                ),
                Err(error) => eprintln!(
                    "NO_REQUIRED_IGNORE_NULL_SOURCE header={header} table={table} recipe={recipe} REFUSED {error:?}"
                ),
            }
            // This is an observation of the current read certificate boundary,
            // not a permanent refusal assertion or a mutable-target token.
            match normal_ordinary::capture(&image, table, &mut budget(), &|| false) {
                Ok(source) => {
                    assert_eq!(source.table.rows(), expected);
                    assert_eq!(source.records.len(), expected.len());
                    for (slot, record) in source.records.iter().enumerate() {
                        assert_eq!(record.reference, ((data as u32) << 8) | slot as u32);
                        let start = data * 4096 + 4096 - (slot + 1) * 19;
                        assert_eq!(record.data, raw[start..start + 19]);
                    }
                    eprintln!(
                        "CANONICAL_IGNORE_NULL_SOURCE header={header} table={table} recipe={recipe} ADMITTED"
                    );
                }
                Err(error) => eprintln!(
                    "CANONICAL_IGNORE_NULL_SOURCE header={header} table={table} recipe={recipe} REFUSED {error:?}"
                ),
            }

            assert_eq!(image.as_bytes(), raw);
            contexts += 1;
        }
    }
    assert_eq!((images.len(), contexts), (8, 12));
}
