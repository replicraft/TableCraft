//! Unchanged unindexed and indexed GUID originals, before normal UPDATE widening.
use crate::{
    GuidIndexSelection, IndexCheckStatus, NativeImage, check_table, inspect_table_index_counter,
    inspect_table_schema, read_table, read_table_guid_index, read_table_index,
    table::mutation::{no_required_programs::NoRequiredTablePrograms, normal_ordinary},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Guid, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_creation as fixed, canonical_ordinary_wide_insert as wide,
};

const SCOPE: CatalogScope = CatalogScope::new(295);
const PAGE: usize = 4096;
const PINS: &[(u8, Option<bool>, &str, &str)] = &[
    (
        5,
        None,
        "h5-fixed-exact-after.accdb",
        "b04d36341ed843e3bf7f2b9bfabd790e9a075d1b8002ad5a53253968c3ea82ed",
    ),
    (
        6,
        None,
        "h6-fixed-exact-after.accdb",
        "1375d5e7453d0603ed42226b05c2f7afa6280f88d63287e2ab73b334a0da339f",
    ),
    (
        5,
        Some(false),
        "V2016-Guid-A-pressure-ordinary-source.accdb",
        "4991ffbc914b22d3ae0769e10a7f4c497e8e3066ce8431735a294e620f2a2353",
    ),
    (
        5,
        Some(true),
        "V2016-Guid-D-pressure-ordinary-source.accdb",
        "04abb49839ea396628571f6e47fb563311db061c082634215a8e175dab82f036",
    ),
    (
        6,
        Some(false),
        "V2019-Guid-A-pressure-ordinary-source.accdb",
        "3ccecfaa6865cecdc932bf239dad0bdfeb770361ef90e0d0d149f91979b1837e",
    ),
    (
        6,
        Some(true),
        "V2019-Guid-D-pressure-ordinary-source.accdb",
        "2cd3572d7ec08f5d538a4733d06edc4d8400838a76765bb0a828b343e4dc681d",
    ),
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn guid(seed: usize) -> Guid {
    let mut bytes = [0_u8; 16];
    bytes[..4].copy_from_slice(&u32::try_from(seed).unwrap().to_be_bytes());
    bytes[4..].copy_from_slice(&[4, 5, 6, 7, 0x80, 9, 10, 11, 12, 13, 14, 0xff]);
    Guid::from_bytes(bytes)
}

fn native_guid(value: Guid) -> [u8; 16] {
    let mut bytes = value.to_bytes();
    bytes[..4].reverse();
    bytes[4..6].reverse();
    bytes[6..8].reverse();
    bytes
}

fn indexed_rows() -> Vec<Vec<Value>> {
    (0..158)
        .map(|slot| {
            vec![
                Value::Int32(slot as i32 + 1),
                if slot == 157 {
                    Value::Null
                } else {
                    Value::Guid(guid(slot))
                },
                Value::Int32(if slot == 157 { 12345 } else { slot as i32 - 64 }),
            ]
        })
        .collect()
}

fn geometry(header: u8) -> (usize, [usize; 2], [usize; 2]) {
    match header {
        5 => (89, [93, 95], [91, 92]),
        6 => (106, [111, 113], [108, 109]),
        _ => panic!("outside the two existing native profiles"),
    }
}

fn record(raw: &[u8], data: usize, slot: usize, width: usize) -> &[u8] {
    let page = data * PAGE;
    let offset = page + 14 + slot * 2;
    let start = usize::from(u16::from_le_bytes(
        raw[offset..offset + 2].try_into().unwrap(),
    ));
    assert_eq!(start, PAGE - width * (slot + 1));
    &raw[page + start..page + start + width]
}

fn indexed_records(raw: &[u8], header: u8) -> Vec<(u32, Vec<u8>)> {
    let (definition, data, _) = geometry(header);
    assert_eq!(
        &raw[definition * PAGE + 16..definition * PAGE + 20],
        &158_u32.to_le_bytes()
    );
    let mut records = Vec::new();
    for (page, count) in [(data[0], 140), (data[1], 18)] {
        assert_eq!(&raw[page * PAGE..page * PAGE + 2], &[1, 1]);
        assert_eq!(
            &raw[page * PAGE + 4..page * PAGE + 8],
            &(definition as u32).to_le_bytes()
        );
        assert_eq!(
            &raw[page * PAGE + 12..page * PAGE + 14],
            &(count as u16).to_le_bytes()
        );
        for slot in 0..count {
            let ordinal = records.len();
            let mut expected = Vec::from(3_u16.to_le_bytes());
            expected.extend_from_slice(&(ordinal as i32 + 1).to_le_bytes());
            // The logical NULL's inactive bytes retain seed156, not zero GUID.
            expected.extend_from_slice(&native_guid(guid(ordinal.min(156))));
            expected.extend_from_slice(
                &(if ordinal == 157 {
                    12345_i32
                } else {
                    ordinal as i32 - 64
                })
                .to_le_bytes(),
            );
            expected.push(if ordinal == 157 { 5 } else { 7 });
            assert_eq!(record(raw, page, slot, 27), expected);
            records.push((((page as u32) << 8) | slot as u32, expected));
        }
    }
    assert_eq!(records.len(), 158);
    records
}

fn entries(
    rows: &[Vec<Value>],
    references: &[(u32, Vec<u8>)],
    secondary: bool,
    descending: bool,
) -> Vec<(Vec<u8>, usize)> {
    let mut entries = Vec::new();
    for (slot, row) in rows.iter().enumerate() {
        let mut wire = if secondary {
            match &row[1] {
                Value::Null => vec![if descending { 0xff } else { 0 }],
                Value::Guid(value) => {
                    let mut bytes = value.to_bytes();
                    if descending {
                        bytes.iter_mut().for_each(|byte| *byte = !*byte);
                    }
                    let mut wire = vec![if descending { 0x80 } else { 0x7f }];
                    wire.extend_from_slice(&bytes[..8]);
                    wire.push(9);
                    wire.extend_from_slice(&bytes[8..]);
                    wire.push(if descending { 0xf7 } else { 8 });
                    wire
                }
                _ => panic!("outside the frozen GUID row law"),
            }
        } else {
            let Value::Int32(id) = row[0] else {
                panic!("manual I32 primary required")
            };
            let mut wire = vec![0x7f];
            wire.extend_from_slice(&((id as u32) ^ 0x8000_0000).to_be_bytes());
            wire
        };
        wire.extend_from_slice(&references[slot].0.to_be_bytes());
        entries.push((wire, slot));
    }
    entries.sort_unstable();
    entries
}

fn leaf(raw: &[u8], definition: usize, root: usize, entries: &[(Vec<u8>, usize)]) {
    let page = &raw[root * PAGE..(root + 1) * PAGE];
    assert_eq!(&page[..2], &[4, 1]);
    assert_eq!(&page[4..8], &(definition as u32).to_le_bytes());
    assert_eq!(&page[8..27], &[0; 19]);
    let mut bitmap = [0_u8; 453];
    let mut active = Vec::new();
    for (wire, _) in entries {
        active.extend_from_slice(wire);
        bitmap[active.len() / 8] |= 1 << (active.len() % 8);
    }
    assert_eq!(&page[27..480], &bitmap);
    assert_eq!(&page[480..480 + active.len()], active);
    assert_eq!(
        &page[2..4],
        &u16::try_from(3616 - active.len()).unwrap().to_le_bytes()
    );
}

#[test]
#[ignore = "six unchanged GUID originals; read/Source observations only, no UPDATE"]
fn existing_guid_originals_keep_data_keys_and_observe_source_admission() {
    let wide_dir =
        PathBuf::from(std::env::var_os("TABLECRAFT_GUID_UPDATE_WIDE_SOURCE_DIR").unwrap());
    let indexed_dir =
        PathBuf::from(std::env::var_os("TABLECRAFT_GUID_UPDATE_INDEX_SOURCE_DIR").unwrap());
    assert!(wide_dir.is_absolute() && indexed_dir.is_absolute());
    let images: Vec<_> = PINS
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
