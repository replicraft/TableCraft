//! Additive literal GUID byte-order witness; existing constructors stay unchanged.
use super::{SCOPE, budget, model, name_map};
use std::{
    fs::OpenOptions,
    io::{Cursor, Write},
    path::PathBuf,
};
use tablecraft_ace::{PropertyGroupKind, inspect_table_properties};

const TABLE: [u8; 16] = [
    0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];
const NUMBER: [u8; 16] = [
    0x78, 0x56, 0x34, 0x12, 0xbc, 0x9a, 0xf0, 0xde, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
];
const YES: [u8; 16] = [
    0xef, 0xcd, 0xab, 0x89, 0x23, 0x01, 0x67, 0x45, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
];
const YES_WRONG_ENDIAN: [u8; 16] = [
    0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
];

fn replace(bytes: &mut [u8], old: &[u8; 16], new: &[u8; 16], count: usize) {
    let positions = bytes
        .windows(16)
        .enumerate()
        .filter_map(|(at, value)| (value == old).then_some(at))
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), count);
    for at in positions {
        bytes[at..at + 16].copy_from_slice(new);
    }
}

pub(super) fn source(swapped_name_map: bool) -> model::Case {
    let mut case = name_map::source(false, false);
    let properties = inspect_table_properties(
        &mut Cursor::new(&case.before),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let original = properties.raw_bytes();
    let mut bag = original.to_vec();
    // Table property plus table record, sixteen column parents and two refs.
    replace(&mut bag, &[0x91; 16], &TABLE, 20);
    // Each field has one property GUID and one NameMap column GUID.
    replace(&mut bag, &[4; 16], &NUMBER, 2);
    replace(&mut bag, &[11; 16], &YES, 2);
    if swapped_name_map {
        let magic = bag
            .windows(4)
            .position(|bytes| bytes == [0x0a, 0xcc, 0x0e, 0x55])
            .unwrap();
        let map_length = properties
            .group(PropertyGroupKind::Table, "")
            .unwrap()
            .get("NameMap")
            .unwrap()
            .raw_value()
            .len();
        replace(
            &mut bag[magic..magic + map_length],
            &YES,
            &YES_WRONG_ENDIAN,
            1,
        );
    }
    for bytes in [&mut case.before, &mut case.expected] {
        for (page, begin, end) in [(23, 0, 2048), (31, 2048, bag.len())] {
            let base = page * 4096;
            assert_eq!(&bytes[base + 4..base + 8], b"LVAL");
            let row = u16::from_le_bytes(bytes[base + 14..base + 16].try_into().unwrap()) as usize;
            let start = base + row + 4;
            assert_eq!(&bytes[start..start + end - begin], &original[begin..end]);
            bytes[start..start + end - begin].copy_from_slice(&bag[begin..end]);
        }
    }
    case
}

fn canonical(native: &[u8]) -> String {
    let order = [3, 2, 1, 0, 5, 4, 7, 6, 8, 9, 10, 11, 12, 13, 14, 15];
    let mut text = String::from("{");
    for (position, byte) in order.into_iter().enumerate() {
        if [4, 6, 8, 10].contains(&position) {
            text.push('-');
        }
        text.push_str(&format!("{:02X}", native[byte]));
    }
    text.push('}');
    text
}

#[test]
fn qualify_preserved_asymmetric_property_and_name_map_guid_sources_before_validator() {
    for swapped in [false, true] {
        let case = source(swapped);
        assert_eq!(
            case.before
                .iter()
                .zip(&case.expected)
                .enumerate()
                .filter_map(
                    |(at, (before, after))| (before != after).then_some((at, before ^ after))
                )
                .collect::<Vec<_>>(),
            [(model::BOOLEAN_PATCH_OFFSET, 4)]
        );
        for bytes in [&case.before, &case.expected] {
            let properties = inspect_table_properties(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            for (kind, name, raw, text) in [
                (
                    PropertyGroupKind::Table,
                    "",
                    TABLE,
                    "{00112233-4455-6677-8899-AABBCCDDEEFF}",
                ),
                (
                    PropertyGroupKind::Column,
                    "Number",
                    NUMBER,
                    "{12345678-9ABC-DEF0-1234-56789ABCDEF0}",
                ),
                (
                    PropertyGroupKind::Column,
                    "Yes_Field",
                    YES,
                    "{89ABCDEF-0123-4567-89AB-CDEF01234567}",
                ),
            ] {
                let actual = properties
                    .group(kind, name)
                    .unwrap()
                    .get("GUID")
                    .unwrap()
                    .raw_value();
                assert_eq!(actual, raw);
                assert_eq!(canonical(actual), text);
            }
            let map = properties
                .group(PropertyGroupKind::Table, "")
                .unwrap()
                .get("NameMap")
                .unwrap()
                .raw_value();
            assert_eq!(map.windows(16).filter(|raw| *raw == TABLE).count(), 19);
            assert_eq!(map.windows(16).filter(|raw| *raw == NUMBER).count(), 1);
            assert_eq!(
                map.windows(16).filter(|raw| *raw == YES).count(),
                usize::from(!swapped)
            );
            assert_eq!(
                map.windows(16)
                    .filter(|raw| *raw == YES_WRONG_ENDIAN)
                    .count(),
                usize::from(swapped)
            );
        }
    }
}

#[test]
#[ignore = "Explicit one-shot asymmetric raw NameMap export for inert independent parsing"]
fn export_preserved_asymmetric_name_maps_for_independent_reader() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(".cache/ace16-preserved-scalar-update-next-20261030/name-map-asymmetric");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, swapped) in [("consistent", false), ("swapped", true)] {
        let case = source(swapped);
        let properties = inspect_table_properties(
            &mut Cursor::new(&case.before),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let map = properties
            .group(PropertyGroupKind::Table, "")
            .unwrap()
            .get("NameMap")
            .unwrap()
            .raw_value();
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(directory.join(format!("{name}.bin")))
            .unwrap();
        file.write_all(map).unwrap();
        file.sync_all().unwrap();
    }
}
