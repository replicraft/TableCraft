//! Read-only SOURCE qualification. No TableCraft creation or mutation producer.
use crate::{NativeImage, check_table, inspect_table_schema, read_table};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_catalog_names as names, canonical_creation as integers,
    canonical_fixed_creation as fixed, canonical_fixed_float_creation as ieee,
    canonical_multiple_creation as multiple, canonical_native17_creation as native17,
    canonical_variable_capacity_creation as capacity, canonical_variable_creation as variable,
};

const SCOPE: CatalogScope = CatalogScope::new(202);

struct Table {
    name: String,
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
}

struct Source {
    id: String,
    before: Vec<u8>,
    tables: Vec<Table>,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn originals() -> Vec<Source> {
    let mut sources = Vec::new();
    for case in integers::CASES {
        let old = integers::source(case);
        sources.push(Source {
            id: format!("i32-{}", case.id()),
            before: old.bytes,
            tables: vec![Table {
                name: old.table.into(),
                columns: old.columns,
                rows: old
                    .rows
                    .into_iter()
                    .map(|row| row.into_iter().map(Value::Int32).collect())
                    .collect(),
            }],
        });
    }
    for case in fixed::CASES {
        let old = fixed::source(case);
        sources.push(Source {
            id: format!("fixed-{}", case.id()),
            before: old.bytes,
            tables: vec![Table {
                name: old.table.into(),
                columns: old
                    .columns
                    .iter()
                    .map(|column| column.name.into())
                    .collect(),
                rows: old.rows,
            }],
        });
    }
    for case in variable::CASES {
        let old = variable::source(case);
        sources.push(Source {
            id: format!("variable-{}", case.id()),
            before: old.bytes,
            tables: vec![Table {
                name: old.table.into(),
                columns: old
                    .columns
                    .iter()
                    .map(|column| column.name.into())
                    .collect(),
                rows: old.rows,
            }],
        });
    }
    for case in multiple::CASES {
        let old = multiple::source(case);
        sources.push(Source {
            id: format!("multiple-{}", case.id()),
            before: old.bytes,
            tables: old
                .tables
                .into_iter()
                .map(|table| Table {
                    name: table.name,
                    columns: table
                        .columns
                        .into_iter()
                        .map(|column| column.name)
                        .collect(),
                    rows: table.rows,
                })
                .collect(),
        });
    }
    for case in ieee::CASES {
        let old = ieee::source(case);
        sources.push(Source {
            id: format!("ieee-{}", case.id()),
            before: old.bytes,
            tables: vec![Table {
                name: old.table.into(),
                columns: old
                    .columns
                    .iter()
                    .map(|column| column.name.into())
                    .collect(),
                rows: old.rows,
            }],
        });
    }
    for case in capacity::CASES {
        let old = capacity::source(case).source;
        sources.push(Source {
            id: format!("capacity-{}", case.id()),
            before: old.bytes,
            tables: vec![Table {
                name: old.table.into(),
                columns: old
                    .columns
                    .iter()
                    .map(|column| column.name.into())
                    .collect(),
                rows: old.rows,
            }],
        });
    }
    for case in names::CASES {
        let old = names::source(case);
        sources.push(Source {
            id: format!("names-{}", case.id()),
            before: old.bytes,
            tables: vec![Table {
                name: old.table,
                columns: old.columns.into_iter().collect(),
                rows: vec![],
            }],
        });
    }
    assert_eq!(sources.len(), 31);
    sources
}

#[test]
fn literal_native17_sources_keep_all_original_user_rows_and_qualify_normal_source() {
    let mut views = 0;
    let mut tables = 0;
    for original in originals() {
        let expected = native17::source(original.before);
        for header in [5_u32, 6] {
            let mut bytes = expected.clone();
            bytes[20..24].copy_from_slice(&header.to_le_bytes());
            let image =
                NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
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
            assert_eq!(catalog.columns()[8].name(), "Database");
            assert_eq!(catalog.columns()[9].name(), "Connect");
            assert_eq!(catalog.columns()[16].name(), "LvExtra");
            for table in &original.tables {
                let observed = read_table(
                    &mut image.reader(),
                    SCOPE,
                    &table.name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    observed
                        .schema()
                        .columns()
                        .iter()
                        .map(|column| column.name())
                        .collect::<Vec<_>>(),
                    table.columns.iter().map(String::as_str).collect::<Vec<_>>()
                );
                assert_eq!(observed.rows(), table.rows);
                let checked = check_table(
                    &mut image.reader(),
                    SCOPE,
                    &table.name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(checked.rows(), table.rows.len());
                let source = super::super::normal_ordinary::capture(
                    &image,
                    &table.name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(source.records.len(), table.rows.len());
                assert_eq!(source.table.rows(), table.rows);
                tables += 1;
            }
            views += 1;
        }
    }
    assert_eq!(views, 62);
    assert_eq!(tables, 98);
}

#[test]
#[ignore = "explicit create-new SOURCE export; no creator or mutation producer"]
fn export_literal_native17_creation_sources() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_NATIVE17_CREATION_EXPORT_DIR").unwrap());
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let mut rows = Vec::new();
    for original in originals() {
        let expected = native17::source(original.before);
        for header in [5_u32, 6] {
            let mut bytes = expected.clone();
            bytes[20..24].copy_from_slice(&header.to_le_bytes());
            let name = format!("h{header}-{}.accdb", original.id);
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join(&name))
                .unwrap();
            std::io::Write::write_all(&mut output, &bytes).unwrap();
            let sha = Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            rows.push(format!("{name}\t{}\t{sha}", bytes.len()));
        }
    }
    assert_eq!(rows.len(), 62);
    fs::write(
        directory.join("manifest.tsv"),
        format!("{}\n", rows.join("\n")),
    )
    .unwrap();
}

fn word(raw: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(raw[at..at + 2].try_into().unwrap()))
}

fn is_rejected(raw: &[u8]) -> bool {
    if let Ok(image) = NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false)
    {
        super::super::normal_ordinary::capture(&image, "Items", &mut budget(), &|| false).is_err()
    } else {
        true
    }
}

#[test]
fn malformed_native17_footer_pairs_and_present_code_never_gain_normal_source() {
    let original = integers::source(integers::Case::One);
    let before = native17::source(original.bytes);
    let td = 2 * 4096;
    let length = u32::from_le_bytes(before[td + 8..td + 12].try_into().unwrap()) as usize;
    let pairs = td + length - 72;
    let data = 4 * 4096;
    let user_start = data + word(&before, data + 14 + 7 * 2);
    let user_end = data + word(&before, data + 14 + 6 * 2);
    assert_eq!(word(&before, user_end - 5), 11);
    assert_eq!(&before[user_end - 3..user_end], &[0xbf, 0, 0]);
    let mut controls = Vec::new();

    let mut short_footer = before.clone();
    short_footer[user_end - 5..user_end - 3].copy_from_slice(&10_u16.to_le_bytes());
    controls.push(short_footer);

    let mut missing_memo_pair = before.clone();
    missing_memo_pair.copy_within(pairs + 10..td + length, pairs);
    missing_memo_pair[td + length - 10..td + length].fill(0);
    missing_memo_pair[td + 8..td + 12].copy_from_slice(&((length - 10) as u32).to_le_bytes());
    let free = word(&before, td + 2) + 10;
    missing_memo_pair[td + 2..td + 4].copy_from_slice(&(free as u16).to_le_bytes());
    controls.push(missing_memo_pair);

    let mut duplicate_pair = before.clone();
    let first = before[pairs + 2..pairs + 6].to_vec();
    duplicate_pair[pairs + 12..pairs + 16].copy_from_slice(&first);
    controls.push(duplicate_pair);

    let mut present_empty_extra = before.clone();
    present_empty_extra[user_end - 1] |= 1;
    controls.push(present_empty_extra);

    let mut wrong_flags = before.clone();
    wrong_flags[td + 63 + 2 * 25 + 15] ^= 0x10;
    controls.push(wrong_flags);

    let mut alias = before.clone();
    // Author a second LIVE local object with a distinct Id/name and the same
    // low24 TD number. All row framing is valid; identity must still refuse.
    let mut rows = Vec::new();
    let mut end = data + 4096;
    for position in 0..8 {
        let start = data + word(&before, data + 14 + 2 * position);
        rows.push((0, before[start..end].to_vec()));
        end = start;
    }
    let mut duplicate = before[user_start..user_end].to_vec();
    assert_eq!(duplicate.len(), 71);
    duplicate[2..6].copy_from_slice(&0x0100_0007_u32.to_le_bytes());
    let name = "Other"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    duplicate[32..42].copy_from_slice(&name);
    rows.push((0, duplicate));
    let payload = rows.iter().map(|(_, row)| row.len()).sum::<usize>();
    let mut page = tablecraft_testkit::fixtures::access::data_page(2, &rows);
    page[2..4].copy_from_slice(&((4096 - 14 - 18 - payload) as u16).to_le_bytes());
    alias[data..data + 4096].copy_from_slice(&page);
    alias[td + 16..td + 20].copy_from_slice(&9_u32.to_le_bytes());
    controls.push(alias);

    assert_eq!(controls.len(), 6);
    let admitted = controls
        .into_iter()
        .enumerate()
        .filter_map(|(index, bad)| (!is_rejected(&bad)).then_some(index))
        .collect::<Vec<_>>();
    assert!(
        admitted.is_empty(),
        "malformed controls admitted: {admitted:?}"
    );
}
