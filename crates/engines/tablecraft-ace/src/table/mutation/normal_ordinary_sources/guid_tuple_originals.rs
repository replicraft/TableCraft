//! Unchanged fitted GUID/I32 tuple originals: complete read/Source qualification.
use crate::{
    IndexCheckStatus, NativeImage, check_table, inspect_table_index_counter, inspect_table_schema,
    read_table, read_table_index,
    table::mutation::{no_required_programs::NoRequiredTablePrograms, normal_ordinary},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Guid, Value},
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(303);
const PAGE: usize = 4096;
const ENTRY_START: usize = 480;
const RECORDS: [&str; 6] = [
    "030001000000000000000504070680090a0b0c0d0eff0a00000007",
    "030002000000000000000504070680090a0b0c0d0eff1400000007",
    "030003000000000000000504070680090a0b0c0d0eff1e00000005",
    "030004000000020000000504070680090a0b0c0d0eff1e00000003",
    "030005000000020000000504070680090a0b0c0d0eff1e00000001",
    "030006000000000000000000000000000000000000000000000007",
];

const CASES: &[Case] = &[
    Case {
        header: 5,
        guid_first: true,
        unique: false,
        ignored: false,
        name: "V2016-GuidThenScore-AD-fitted-ordinary-source.accdb",
        sha256: "6db68d365572ccaca857cfa382352a5d2e84e7b4d2e2749580adc1b4880eae6f",
    },
    Case {
        header: 5,
        guid_first: true,
        unique: true,
        ignored: true,
        name: "V2016-GuidThenScore-AD-fitted-unique-ignored-source.accdb",
        sha256: "7bf2a7f850d251c5bf05ba2b588c420cc80b9c1945d3724649fd388229a8a84a",
    },
    Case {
        header: 5,
        guid_first: true,
        unique: true,
        ignored: false,
        name: "V2016-GuidThenScore-AD-fitted-unique-source.accdb",
        sha256: "8e43c16c89129ea941bda825025dab0f9142aeed6c6b929a0083bc1cd3286c96",
    },
    Case {
        header: 5,
        guid_first: false,
        unique: false,
        ignored: false,
        name: "V2016-ScoreThenGuid-AD-fitted-ordinary-source.accdb",
        sha256: "faa5ec71349b21353449260a47f27832a13d77414b53bf96d7ec7a2773e8e4ca",
    },
    Case {
        header: 5,
        guid_first: false,
        unique: true,
        ignored: true,
        name: "V2016-ScoreThenGuid-AD-fitted-unique-ignored-source.accdb",
        sha256: "83421aac0d51714f4293a013364bb71c0612659c93d33dbf0f9838c3a1e7e9f2",
    },
    Case {
        header: 5,
        guid_first: false,
        unique: true,
        ignored: false,
        name: "V2016-ScoreThenGuid-AD-fitted-unique-source.accdb",
        sha256: "d39afd860d4b6e6072e575b29293c1a62f9e829c8cf5235dbd5ae6e882d65958",
    },
    Case {
        header: 6,
        guid_first: true,
        unique: false,
        ignored: false,
        name: "V2019-GuidThenScore-AD-fitted-ordinary-source.accdb",
        sha256: "b2b7eb48bc27933f6f12a7f704fd68cb20322c827150a42edc93b24b74fac565",
    },
    Case {
        header: 6,
        guid_first: true,
        unique: true,
        ignored: true,
        name: "V2019-GuidThenScore-AD-fitted-unique-ignored-source.accdb",
        sha256: "8dc4610dd522482ae24f0ada1b81a031e6820562800485718fe2be2dccf860d2",
    },
    Case {
        header: 6,
        guid_first: true,
        unique: true,
        ignored: false,
        name: "V2019-GuidThenScore-AD-fitted-unique-source.accdb",
        sha256: "3def0fe4cea48ca7e7d4e8da16bb37829ee6e43301d0c23bf840c56201891c3d",
    },
    Case {
        header: 6,
        guid_first: false,
        unique: false,
        ignored: false,
        name: "V2019-ScoreThenGuid-AD-fitted-ordinary-source.accdb",
        sha256: "0f0e5f0bd500dc242e8ee9459c737eb660135cd20ba875e6649ecd69747dd565",
    },
    Case {
        header: 6,
        guid_first: false,
        unique: true,
        ignored: true,
        name: "V2019-ScoreThenGuid-AD-fitted-unique-ignored-source.accdb",
        sha256: "9e6d9c446a4bd324b7ff430e95c0e7720b513cc79cbe593dfeef3ee75b832f30",
    },
    Case {
        header: 6,
        guid_first: false,
        unique: true,
        ignored: false,
        name: "V2019-ScoreThenGuid-AD-fitted-unique-source.accdb",
        sha256: "1f4625574b692fddca25f9cc45cb058add8015d3ecbe0c01b38b738958e15beb",
    },
];

struct Case {
    header: u8,
    guid_first: bool,
    unique: bool,
    ignored: bool,
    name: &'static str,
    sha256: &'static str,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn guid(seed: u32) -> Guid {
    let mut value = [0; 16];
    value[..4].copy_from_slice(&seed.to_be_bytes());
    value[4..].copy_from_slice(&[4, 5, 6, 7, 128, 9, 10, 11, 12, 13, 14, 255]);
    Guid::from_bytes(value)
}

fn rows() -> Vec<Vec<Value>> {
    vec![
        vec![Value::Int32(1), Value::Guid(guid(0)), Value::Int32(10)],
        vec![Value::Int32(2), Value::Guid(guid(0)), Value::Int32(20)],
        vec![Value::Int32(3), Value::Null, Value::Int32(30)],
        vec![Value::Int32(4), Value::Guid(guid(2)), Value::Null],
        vec![Value::Int32(5), Value::Null, Value::Null],
        vec![
            Value::Int32(6),
            Value::Guid(Guid::from_bytes([0; 16])),
            Value::Int32(0),
        ],
    ]
}

fn component(value: &Value, descending: bool) -> Vec<u8> {
    match value {
        Value::Null => vec![if descending { 255 } else { 0 }],
        Value::Guid(value) => {
            let mut data = value.to_bytes();
            if descending {
                data.iter_mut().for_each(|byte| *byte = !*byte);
            }
            let mut result = vec![if descending { 128 } else { 127 }];
            result.extend_from_slice(&data[..8]);
            result.push(9);
            result.extend_from_slice(&data[8..]);
            result.push(if descending { 247 } else { 8 });
            result
        }
        Value::Int32(value) => {
            let mut result = vec![127];
            result.extend_from_slice(&((*value as u32) ^ 0x8000_0000).to_be_bytes());
            if descending {
                result.iter_mut().for_each(|byte| *byte = !*byte);
            }
            result
        }
        _ => panic!("outside the original GUID/I32 tuple law"),
    }
}

fn ordered(rows: &[Vec<Value>], data: u32, case: &Case, primary: bool) -> Vec<(Vec<u8>, usize)> {
    let mut result = Vec::new();
    for (slot, row) in rows.iter().enumerate() {
        if !primary && case.ignored && row[1] == Value::Null && row[2] == Value::Null {
            continue;
        }
        let mut wire = if primary {
            component(&row[0], false)
        } else {
            let columns = if case.guid_first { [1, 2] } else { [2, 1] };
            let mut key = component(&row[columns[0]], false);
            key.extend(component(&row[columns[1]], true));
            key
        };
        wire.extend_from_slice(&((data << 8) | slot as u32).to_be_bytes());
        result.push((wire, slot));
    }
    result.sort_unstable();
    result
}

fn raw_leaf(raw: &[u8], root: usize, definition: u32, entries: &[(Vec<u8>, usize)]) {
    let page = &raw[root * PAGE..(root + 1) * PAGE];
    assert_eq!(&page[..2], &[4, 1]);
    assert_eq!(&page[4..8], &definition.to_le_bytes());
    assert_eq!(&page[8..27], &[0; 19]);
    let mut active = Vec::new();
    let mut bitmap = [0_u8; 453];
    for (wire, _) in entries {
        active.extend_from_slice(wire);
        let end = active.len();
        bitmap[end / 8] |= 1 << (end % 8);
    }
    assert_eq!(&page[27..ENTRY_START], &bitmap);
    assert_eq!(&page[ENTRY_START..ENTRY_START + active.len()], active);
    assert_eq!(
        &page[2..4],
        &u16::try_from(3616 - active.len()).unwrap().to_le_bytes()
    );
}

#[test]
#[ignore = "12 unchanged SDK tuple originals; read/Source diagnostics, no mutation"]
fn fitted_guid_tuples_keep_complete_eligible_membership_and_observe_source() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_GUID_TUPLE_UPDATE_ORIGINAL_DIR").unwrap());
    assert!(directory.is_absolute());
    let images: Vec<_> = CASES
        .iter()
        .map(|case| {
            let raw = fs::read(directory.join(case.name)).unwrap();
            assert_eq!(digest(&raw), case.sha256);
            raw
        })
        .collect();
    let expected = rows();
    let mut admitted = [0_usize; 2];
    for (case, raw) in CASES.iter().zip(&images) {
        let (definition, data, roots) = if case.header == 5 {
            (89_u32, 93_u32, [91, 92])
        } else {
            assert_eq!(case.header, 6);
            (106, 111, [108, 109])
        };
        assert_eq!(raw.len(), if case.header == 5 { 95 } else { 113 } * PAGE);
        let td = definition as usize * PAGE;
        assert_eq!(&raw[td + 16..td + 20], &6_u32.to_le_bytes());
        let secondary_count = if case.ignored { 5_u32 } else { 6 };
        for (offset, count) in [(67, 6_u32), (79, secondary_count)] {
            assert_eq!(&raw[td + offset..td + offset + 4], &count.to_le_bytes());
        }
        let data_start = data as usize * PAGE;
        assert_eq!(&raw[data_start..data_start + 2], &[1, 1]);
        assert_eq!(
            &raw[data_start + 4..data_start + 8],
            &definition.to_le_bytes()
        );
        assert_eq!(&raw[data_start + 12..data_start + 14], &6_u16.to_le_bytes());
        for (slot, literal) in RECORDS.iter().enumerate() {
            let word = data_start + 14 + slot * 2;
            let offset = PAGE - 27 * (slot + 1);
            assert_eq!(
                &raw[word..word + 2],
                &u16::try_from(offset).unwrap().to_le_bytes()
            );
            let observed: String = raw[data_start + offset..data_start + offset + 27]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(&observed, literal);
        }
        let keys = [
            ordered(&expected, data, case, true),
            ordered(&expected, data, case, false),
        ];
        for (root, entries) in roots.iter().zip(&keys) {
            raw_leaf(raw, *root, definition, entries);
        }

        let image =
            NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
        let schema =
            inspect_table_schema(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(schema.definition_page(), definition);
        assert_eq!(schema.columns().len(), 3);
        for (position, field) in schema.columns().iter().enumerate() {
            assert_eq!(field.number(), position);
            assert_eq!(field.name(), ["ID", "Group", "Score"][position]);
            assert_eq!(field.native_type(), [4, 15, 4][position]);
            assert_eq!(field.storage_length(), [4, 16, 4][position]);
            assert!(field.fixed());
            assert_eq!((field.flags(), field.extended_flags()), (3, 0));
        }
        let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(table.rows(), expected);
        let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(checked.rows(), 6);
        assert_eq!(checked.indexes().len(), 4);
        let mut aliases = [0; 2];
        let mut maps = [None; 2];
        for checked in checked.indexes() {
            assert_eq!(checked.status(), IndexCheckStatus::Checked);
            let index = checked.index();
            let secondary = match index.name() {
                "PrimaryID" | "PrimaryAlias" => false,
                "GroupOrder" | "GroupAlias" => true,
                _ => panic!("outside four actual aliases"),
            };
            let position = usize::from(secondary);
            aliases[position] += 1;
            assert_eq!(index.root_page() as usize, roots[position]);
            assert_eq!(index.data_number(), position as u32);
            if let Some(prior) = maps[position] {
                assert_eq!(index.usage_map(), prior);
            } else {
                maps[position] = Some(index.usage_map());
            }
            let columns = if secondary {
                if case.guid_first {
                    vec![1, 2]
                } else {
                    vec![2, 1]
                }
            } else {
                vec![0]
            };
            assert_eq!(index.columns().len(), columns.len());
            for (ordinal, key) in index.columns().iter().enumerate() {
                assert_eq!(key.number(), columns[ordinal]);
                assert_eq!(key.descending(), secondary && ordinal == 1);
            }
            let flags = index.constraints().unwrap();
            assert_eq!(flags.primary_key(), index.name() == "PrimaryID");
            assert_eq!(flags.unique(), !secondary || case.unique);
            assert_eq!(flags.required(), !secondary);
            assert_eq!(flags.ignore_nulls(), secondary && case.ignored);
            let history = inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                "Items",
                index.name(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(history.value(), if secondary { secondary_count } else { 6 });
            // Complete All is intentionally not misused on an omitted tuple.
            // check_table already proved its entire eligible sequential domain.
            if !secondary || !case.ignored {
                let read = read_table_index(
                    &mut image.reader(),
                    SCOPE,
                    "Items",
                    index.name(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let ordered_rows: Vec<_> = keys[position]
                    .iter()
                    .map(|(_, row)| expected[*row].clone())
                    .collect();
                assert_eq!(read.rows(), ordered_rows);
            }
        }
        assert_eq!(aliases, [2, 2]);
        assert_ne!(maps[0], maps[1]);
        match NoRequiredTablePrograms::capture_updates(&image, &schema, &mut budget(), &|| false) {
            Ok(_) => {
                admitted[0] += 1;
                eprintln!("GUID_TUPLE_NO_REQUIRED {} ADMITTED", case.name);
            }
            Err(error) => panic!("GUID_TUPLE_NO_REQUIRED {} REFUSED {error:?}", case.name),
        }
        match normal_ordinary::capture(&image, "Items", &mut budget(), &|| false) {
            Ok(source) => {
                admitted[1] += 1;
                assert_eq!(source.table.rows(), expected);
                assert_eq!(source.records.len(), 6);
                for (slot, record) in source.records.iter().enumerate() {
                    assert_eq!(record.reference, (data << 8) | slot as u32);
                    let start = data_start + PAGE - 27 * (slot + 1);
                    assert_eq!(record.data, raw[start..start + 27]);
                }
                eprintln!("GUID_TUPLE_CANONICAL_SOURCE {} ADMITTED", case.name);
            }
            Err(error) => panic!(
                "GUID_TUPLE_CANONICAL_SOURCE {} REFUSED {error:?}",
                case.name
            ),
        }
        assert_eq!(image.as_bytes(), raw);
    }
    assert_eq!(images.len(), 12);
    assert_eq!(admitted, [12, 12]);
    eprintln!(
        "GUID_TUPLE_SOURCE_SUMMARY views12 NoRequiredAdmitted{} SourceAdmitted{}",
        admitted[0], admitted[1]
    );
}
