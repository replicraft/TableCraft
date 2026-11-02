//! Read-only original qualification; no planner, encoder or candidate call.
use crate::{
    IndexCheckStatus, NativeImage, check_table_constraints, inspect_table_index_counter,
    read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

const SCOPE: CatalogScope = CatalogScope::new(269);

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 10_000_000, 2_000_000_000)
}
fn hex(raw: &[u8]) -> String {
    raw.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn wide(value: Option<i64>, money: bool) -> Value {
    match value {
        None => Value::Null,
        Some(value) if money => Value::Currency(Currency::from_scaled(value)),
        Some(value) => Value::Int64(value),
    }
}
struct Table {
    name: &'static str,
    definition: u32,
    data: u32,
    roots: [u32; 3],
    money: bool,
    primary: bool,
}
struct Case {
    path: &'static str,
    bytes: usize,
    hash: &'static str,
    tables: [Table; 2],
}

fn cases() -> [Case; 4] {
    [
        Case {
            path: ".cache/ace16-normal-integral-indexed-next-20261101/source/original-first/originals/h5-integral-indexed-original.accdb",
            bytes: 421888,
            hash: "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1",
            tables: [
                Table {
                    name: "Int64Keys",
                    definition: 89,
                    data: 94,
                    roots: [91, 92, 93],
                    money: false,
                    primary: false,
                },
                Table {
                    name: "CurrencyKeys",
                    definition: 96,
                    data: 101,
                    roots: [98, 99, 100],
                    money: true,
                    primary: false,
                },
            ],
        },
        Case {
            path: ".cache/ace16-normal-integral-indexed-next-20261101/source/original-first/originals/h6-integral-indexed-original.accdb",
            bytes: 491520,
            hash: "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e",
            tables: [
                Table {
                    name: "Int64Keys",
                    definition: 106,
                    data: 111,
                    roots: [108, 109, 110],
                    money: false,
                    primary: false,
                },
                Table {
                    name: "CurrencyKeys",
                    definition: 113,
                    data: 118,
                    roots: [115, 116, 117],
                    money: true,
                    primary: false,
                },
            ],
        },
        Case {
            path: ".cache/ace16-normal-integral-indexed-next-20261101/extra-originals/original-first/originals/h5-integral-extra-original.accdb",
            bytes: 540672,
            hash: "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            tables: [
                Table {
                    name: "Int64WidePrimary",
                    definition: 103,
                    data: 108,
                    roots: [105, 106, 107],
                    money: false,
                    primary: true,
                },
                Table {
                    name: "CurrencyWidePrimary",
                    definition: 110,
                    data: 115,
                    roots: [112, 113, 114],
                    money: true,
                    primary: true,
                },
            ],
        },
        Case {
            path: ".cache/ace16-normal-integral-indexed-next-20261101/extra-originals/original-first/originals/h6-integral-extra-original.accdb",
            bytes: 610304,
            hash: "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            tables: [
                Table {
                    name: "Int64WidePrimary",
                    definition: 120,
                    data: 126,
                    roots: [122, 123, 124],
                    money: false,
                    primary: true,
                },
                Table {
                    name: "CurrencyWidePrimary",
                    definition: 128,
                    data: 133,
                    roots: [130, 131, 132],
                    money: true,
                    primary: true,
                },
            ],
        },
    ]
}

#[test]
#[ignore = "requires the four immutable SDK integral and wider-primary original files"]
fn wider_indexed_update_originals_require_actual_native17_source_and_all_alias_reads() {
    let root = PathBuf::from(std::env::var_os("TABLECRAFT_WIDE_UPDATE_SOURCE_REPO").unwrap());
    assert!(root.is_absolute());
    let mut verified = Vec::new();
    for case in cases() {
        let raw = fs::read(root.join(case.path)).unwrap();
        assert_eq!(raw.len(), case.bytes);
        assert_eq!(hex(&Sha256::digest(&raw)), case.hash);
        verified.push((case, raw));
    }
    for (case, raw) in verified {
        let image =
            NativeImage::capture(&mut Cursor::new(&raw), SCOPE, &mut budget(), &|| false).unwrap();
        let catalog = crate::inspect_table_schema(
            &mut image.reader(),
            SCOPE,
            "MSysObjects",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(catalog.columns().len(), 17);
        for table in case.tables {
            let source = super::capture(&image, table.name, &mut budget(), &|| false).unwrap();
            let mut expected = Vec::new();
            for (ordinal, key) in [Some(i64::MIN), None, Some(i64::MAX)]
                .into_iter()
                .enumerate()
            {
                let id = if table.primary {
                    wide(Some([i64::MIN, 0, i64::MAX][ordinal]), table.money)
                } else {
                    Value::Int32(ordinal as i32 + 1)
                };
                expected.push(vec![
                    id,
                    wide(key, table.money),
                    Value::Int32([17, 34, 51][ordinal]),
                ]);
            }
            assert_eq!(source.table.rows(), expected);
            assert_eq!(source.table.schema().definition_page(), table.definition);
            assert_eq!(source.table.schema().declared_rows(), 3);
            assert_eq!(source.records.len(), 3);
            let fields = source.table.schema().columns();
            assert_eq!(fields.len(), 3);
            for (position, field) in fields.iter().enumerate() {
                assert_eq!(field.number(), position);
                assert_eq!(field.flags(), 3);
                assert_eq!(field.extended_flags(), 0);
                assert!(field.fixed());
                let wider = position == 1 || position == 0 && table.primary;
                assert_eq!(field.storage_length(), if wider { 8 } else { 4 });
                assert_eq!(
                    field.scalar_type(),
                    Some(if wider {
                        if table.money {
                            ScalarType::Currency
                        } else {
                            ScalarType::Int64
                        }
                    } else {
                        ScalarType::Int32
                    })
                );
            }
            let record_length = if table.primary { 23 } else { 19 };
            for (ordinal, record) in source.records.iter().enumerate() {
                let start = 4096 - record_length * (ordinal + 1);
                let end = 4096 - record_length * ordinal;
                assert_eq!(record.reference, table.data << 8 | ordinal as u32);
                assert_eq!(record.offset, table.data as usize * 4096 + start);
                assert_eq!(
                    record.data,
                    raw[table.data as usize * 4096 + start..table.data as usize * 4096 + end]
                );
                assert_eq!(&record.data[..2], &[3, 0]);
                assert_eq!(
                    record.data[record_length - 1],
                    if ordinal == 1 { 5 } else { 7 }
                );
            }
            let checked = crate::table::indexes::check_table_retained(
                &mut image.reader(),
                SCOPE,
                table.name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(checked.rows(), 3);
            assert_eq!(checked.indexes().len(), 6);
            for (physical, aliases) in [
                ["PrimaryID", "PrimaryAlias"],
                ["Ascending", "AscendingAlias"],
                ["Descending", "DescendingAlias"],
            ]
            .into_iter()
            .enumerate()
            {
                for alias in aliases {
                    let index = checked
                        .indexes()
                        .iter()
                        .find(|entry| entry.index().name() == alias)
                        .unwrap();
                    assert_eq!(index.status(), IndexCheckStatus::Checked);
                    assert_eq!(index.index().root_page(), table.roots[physical]);
                    assert_eq!(index.index().data_number(), physical as u32);
                    let counter = inspect_table_index_counter(
                        &mut image.reader(),
                        SCOPE,
                        table.name,
                        alias,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(counter.value(), 3);
                    let read = read_table_index(
                        &mut image.reader(),
                        SCOPE,
                        table.name,
                        alias,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let order = match physical {
                        0 => [0, 1, 2],
                        1 => [1, 0, 2],
                        2 => [2, 0, 1],
                        _ => unreachable!(),
                    };
                    assert_eq!(
                        read.rows(),
                        order
                            .into_iter()
                            .map(|position| expected[position].clone())
                            .collect::<Vec<_>>()
                    );
                }
            }
            let read = read_table(
                &mut image.reader(),
                SCOPE,
                table.name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(read.rows(), expected);
            let constraints = check_table_constraints(
                &mut image.reader(),
                SCOPE,
                table.name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(constraints.rows(), 3);
            assert!(constraints.relationships().is_empty());
        }
        assert_eq!(hex(&Sha256::digest(image.as_bytes())), case.hash);
    }
}
