//! Read-only literal source qualification; no creator, resolver or INSERT calls.
use crate::{
    NativeImage, PropertyGroupKind, check_table, check_table_constraints, inspect_table_properties,
    read_table,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(226);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[derive(Clone, Copy)]
enum Case {
    Constants,
    FalseMaxQuoted,
    Nullable,
    RequiredEmpty,
}

const CASES: [Case; 4] = [
    Case::Constants,
    Case::FalseMaxQuoted,
    Case::Nullable,
    Case::RequiredEmpty,
];

impl Case {
    fn id(self) -> &'static str {
        match self {
            Self::Constants => "constants",
            Self::FalseMaxQuoted => "false-max-quoted",
            Self::Nullable => "nullable-defaults",
            Self::RequiredEmpty => "required-empty",
        }
    }

    fn expressions(self) -> [&'static str; 4] {
        match self {
            Self::Constants => ["True", "-9223372036854775808", "-1.0001", "\"seed\""],
            Self::FalseMaxQuoted => [
                "False",
                "9223372036854775807",
                "922337203685477.5807",
                "\"a\"\"bΔ\"",
            ],
            Self::Nullable => ["True", "Null", "Null", "Null"],
            Self::RequiredEmpty => ["False", "-1", "0.0000", "\"\""],
        }
    }

    fn defaults(self) -> [Value; 4] {
        match self {
            Self::Constants => [
                Value::Boolean(true),
                Value::Int64(i64::MIN),
                Value::Currency(Currency::from_scaled(-10001)),
                Value::Text("seed".into()),
            ],
            Self::FalseMaxQuoted => [
                Value::Boolean(false),
                Value::Int64(i64::MAX),
                Value::Currency(Currency::from_scaled(i64::MAX)),
                Value::Text("a\"bΔ".into()),
            ],
            Self::Nullable => [Value::Boolean(true), Value::Null, Value::Null, Value::Null],
            Self::RequiredEmpty => [
                Value::Boolean(false),
                Value::Int64(-1),
                Value::Currency(Currency::from_scaled(0)),
                Value::Text(String::new()),
            ],
        }
    }

    fn bag_bytes(self) -> usize {
        match self {
            Self::Constants => 273,
            Self::FalseMaxQuoted => 301,
            Self::Nullable => 231,
            Self::RequiredEmpty => 256,
        }
    }
}

fn original_row() -> Vec<Value> {
    vec![
        Value::Int32(1),
        Value::Boolean(false),
        Value::Int64(7),
        Value::Currency(Currency::from_scaled(25000)),
        Value::Text("original".into()),
    ]
}

fn rows(case: Case, phase: &str) -> Option<Vec<Vec<Value>>> {
    let mut result = vec![original_row()];
    if phase == "before" {
        return Some(result);
    }
    if matches!(case, Case::RequiredEmpty) && phase != "supplied-overrides" {
        return None;
    }
    let [flag, large, money, caption] = case.defaults();
    let next = match phase {
        "omitted-constants" => vec![Value::Int32(2), flag, large, money, caption],
        "supplied-null" => vec![Value::Int32(3), flag, Value::Null, Value::Null, Value::Null],
        "supplied-overrides" => vec![
            Value::Int32(4),
            Value::Boolean(false),
            large,
            money,
            Value::Text("override".into()),
        ],
        _ => panic!("frozen request phase"),
    };
    result.push(next);
    Some(result)
}

fn record(row: &[Value]) -> Vec<u8> {
    let [
        Value::Int32(id),
        Value::Boolean(flag),
        large,
        money,
        caption,
    ] = row
    else {
        panic!("complete independently literal carrier row");
    };
    let mut result = 5_u16.to_le_bytes().to_vec();
    result.extend(id.to_le_bytes());
    let mut mask = 1 | if *flag { 2 } else { 0 };
    if let Value::Int64(value) = large {
        result.extend(value.to_le_bytes());
        mask |= 4;
    } else {
        assert_eq!(large, &Value::Null);
        result.extend([0; 8]);
    }
    if let Value::Currency(value) = money {
        result.extend(value.scaled().to_le_bytes());
        mask |= 8;
    } else {
        assert_eq!(money, &Value::Null);
        result.extend([0; 8]);
    }
    assert_eq!(result.len(), 22);
    if let Value::Text(value) = caption {
        result.extend(value.encode_utf16().flat_map(u16::to_le_bytes));
        mask |= 16;
    } else {
        assert_eq!(caption, &Value::Null);
    }
    let end = result.len();
    result.extend((end as u16).to_le_bytes());
    result.extend(22_u16.to_le_bytes());
    result.extend(1_u16.to_le_bytes());
    result.push(mask);
    result
}

fn check(raw: &[u8], case: Case, expected: &[Vec<Value>]) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    assert_eq!(raw.len(), 9 * 4096);
    let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(table.rows(), expected);
    assert_eq!(
        table
            .schema()
            .columns()
            .iter()
            .map(|field| field.name())
            .collect::<Vec<_>>(),
        ["Id", "Flag", "Large", "Money", "Caption"]
    );
    assert_eq!(
        table
            .schema()
            .columns()
            .iter()
            .map(|field| field.native_type())
            .collect::<Vec<_>>(),
        [4, 1, 19, 5, 10]
    );
    assert_eq!(
        table
            .schema()
            .columns()
            .iter()
            .map(|field| field.storage_length())
            .collect::<Vec<_>>(),
        [4, 0, 8, 8, 32]
    );
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), expected.len());
    assert!(checked.indexes().is_empty());
    check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    let properties =
        inspect_table_properties(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(properties.raw_bytes().len(), case.bag_bytes());
    crate::properties::framing::validate(&properties, &mut budget(), &|| false).unwrap();
    assert_eq!(properties.groups().len(), 4);
    for (position, (name, expression)) in ["Flag", "Large", "Money", "Caption"]
        .into_iter()
        .zip(case.expressions())
        .enumerate()
    {
        let group = &properties.groups()[position];
        assert_eq!(group.kind(), PropertyGroupKind::Column);
        assert_eq!(group.name(), name);
        let actual = group.get("DefaultValue").unwrap();
        assert_eq!((actual.native_type(), actual.flags()), (12, 1));
        assert_eq!(actual.text().unwrap(), expression);
        assert_eq!(
            actual.raw_value(),
            expression
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>()
        );
        if name == "Caption" {
            let allow = group.get("AllowZeroLength").unwrap();
            assert_eq!((allow.native_type(), allow.flags()), (1, 1));
            assert_eq!(
                allow.boolean().unwrap(),
                !matches!(case, Case::RequiredEmpty)
            );
            assert_eq!(
                allow.raw_value(),
                &[if matches!(case, Case::RequiredEmpty) {
                    0
                } else {
                    255
                }]
            );
            assert_eq!(
                group.properties().len(),
                if matches!(case, Case::RequiredEmpty) {
                    3
                } else {
                    2
                }
            );
            if matches!(case, Case::RequiredEmpty) {
                assert!(group.get("Required").unwrap().boolean().unwrap());
            } else {
                assert!(group.get("Required").is_none());
            }
        } else {
            assert_eq!(group.properties().len(), 1);
        }
    }
    let source =
        super::super::normal_ordinary::capture(&image, "Items", &mut budget(), &|| false).unwrap();
    assert_eq!(source.table.rows(), expected);
    let page = &raw[8 * 4096..9 * 4096];
    assert_eq!(
        usize::from(u16::from_le_bytes(page[12..14].try_into().unwrap())),
        expected.len()
    );
    let mut end = 4096;
    for (slot, row) in expected.iter().enumerate() {
        let at = 14 + 2 * slot;
        let start = usize::from(u16::from_le_bytes(page[at..at + 2].try_into().unwrap()));
        assert_eq!(&page[start..end], record(row));
        end = start;
    }
    assert_eq!(
        usize::from(u16::from_le_bytes(page[2..4].try_into().unwrap())),
        end - 14 - 2 * expected.len()
    );
}

fn read(path: &Path, binding: &(usize, String)) -> Vec<u8> {
    assert!(fs::symlink_metadata(path).unwrap().file_type().is_file());
    assert_eq!(fs::metadata(path).unwrap().len(), 9 * 4096);
    let raw = fs::read(path).unwrap();
    assert_eq!(raw.len(), binding.0);
    let sha = Sha256::digest(&raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(sha, binding.1);
    raw
}

#[test]
#[ignore = "requires pure frozen constant-default source28 construction before matching creators"]
fn literal_constant_default_sources_preserve_metadata_presence_values_and_whole_bytes() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_CONSTANT_DEFAULT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let manifest = fs::read_to_string(directory.join("manifest.tsv")).unwrap();
    let mut bindings = BTreeMap::new();
    for line in manifest.lines() {
        let parts = line.split('\t').collect::<Vec<_>>();
        assert_eq!(parts.len(), 3);
        assert!(
            bindings
                .insert(
                    parts[0].to_owned(),
                    (parts[1].parse::<usize>().unwrap(), parts[2].to_owned())
                )
                .is_none()
        );
    }
    assert_eq!(bindings.len(), 28);
    let mut views = 0;
    for header in [5_u32, 6] {
        for case in CASES {
            let name = format!("h{header}-{}-before.accdb", case.id());
            let before = read(&directory.join(&name), bindings.get(&name).unwrap());
            assert_eq!(&before[20..24], header.to_le_bytes());
            check(&before, case, &rows(case, "before").unwrap());
            views += 1;
            for phase in ["omitted-constants", "supplied-null", "supplied-overrides"] {
                let name = format!("h{header}-{}-{phase}.accdb", case.id());
                let Some(expected) = rows(case, phase) else {
                    assert!(!bindings.contains_key(&name));
                    assert!(!directory.join(&name).exists());
                    continue;
                };
                let after = read(&directory.join(&name), bindings.get(&name).unwrap());
                check(&after, case, &expected);
                let length = record(expected.last().unwrap()).len();
                let start = 4051 - length;
                for at in 0..before.len() {
                    let mutable = (7 * 4096 + 16..7 * 4096 + 20).contains(&at)
                        || (8 * 4096 + 2..8 * 4096 + 4).contains(&at)
                        || (8 * 4096 + 12..8 * 4096 + 14).contains(&at)
                        || (8 * 4096 + 16..8 * 4096 + 18).contains(&at)
                        || (8 * 4096 + start..8 * 4096 + 4051).contains(&at);
                    if !mutable {
                        assert_eq!(before[at], after[at], "preserved literal source byte {at}");
                    }
                }
                views += 1;
            }
        }
    }
    assert_eq!(views, 28);
}
