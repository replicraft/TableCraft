//! Independent Name AutoCorrect derivative, before any preservation writer.
use super::{SCOPE, budget, catalog_flag, model, support};
use std::{
    fs::OpenOptions,
    io::{Cursor, Write},
    path::PathBuf,
};
use tablecraft_ace::{
    PropertyGroupKind, inspect_table_properties, inspect_table_schema, read_attachment_column,
    read_stored_int32_calculation,
};

const FIELDS: [&str; 16] = [
    "ID",
    "Short_Text",
    "Long_Text",
    "Number",
    "Large_Number",
    "Date_Field",
    "Time_Field",
    "Date_Extended_Field",
    "Time_Extended_Field",
    "Currency_Field",
    "Yes_Field",
    "No_Field",
    "OLE_Object_Field",
    "Hyperlink_Field",
    "Attachment_Field",
    "Calculated",
];

fn text(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn literal_map(stale: bool) -> Vec<u8> {
    let mut bytes = vec![0x0a, 0xcc, 0x0e, 0x55, 0, 0, 0, 0];
    let mut records = vec![([0x91; 16], 0_u32, [0; 16], "Items")];
    for (position, name) in FIELDS.into_iter().enumerate() {
        records.push((
            [position as u8 + 1; 16],
            7,
            [0x91; 16],
            if stale && position == 0 {
                "OLD_ID"
            } else {
                name
            },
        ));
    }
    for name in ["ID", "Number"] {
        records.push(([0; 16], 6, [0x91; 16], name));
    }
    let mut writer = [0; 16];
    writer[0] = 5;
    records.push(([0; 16], 12, writer, ""));
    for (position, (guid, kind, payload, name)) in records.iter().enumerate() {
        bytes.extend(guid);
        bytes.extend(kind.to_le_bytes());
        bytes.extend(payload);
        bytes.extend(text(name));
        bytes.extend([0; 2]);
        if position + 1 < records.len() {
            bytes.extend([0; 4]);
        }
    }
    bytes
}

fn label(name: &str) -> Vec<u8> {
    let value = text(name);
    [(value.len() as u16).to_le_bytes().as_slice(), &value].concat()
}

fn block(kind: u16, payload: &[u8]) -> Vec<u8> {
    [
        ((payload.len() + 6) as u32).to_le_bytes().as_slice(),
        &kind.to_le_bytes(),
        payload,
    ]
    .concat()
}

fn value(index: usize, kind: u8, flags: u8, raw: &[u8]) -> Vec<u8> {
    [
        ((raw.len() + 8) as u16).to_le_bytes().as_slice(),
        &[flags, kind],
        &(index as u16).to_le_bytes(),
        &(raw.len() as u16).to_le_bytes(),
        raw,
    ]
    .concat()
}

fn bag(before: &[u8], missing_attachment_guid: bool, stale: bool) -> Vec<u8> {
    let properties = inspect_table_properties(
        &mut Cursor::new(before),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut names = Vec::new();
    for group in properties.groups() {
        for property in group.properties() {
            if !names.contains(&property.name()) {
                names.push(property.name());
            }
        }
    }
    let map_index = names.len();
    names.push("NameMap");
    let mut result = b"MR2\0".to_vec();
    result.extend(block(
        0x80,
        &names
            .iter()
            .flat_map(|name| label(name))
            .collect::<Vec<_>>(),
    ));
    for group in properties.groups() {
        let name = label(group.name());
        let mut body = ((name.len() + 4) as u32).to_le_bytes().to_vec();
        body.extend(name);
        for property in group.properties() {
            if missing_attachment_guid
                && group.name() == "Attachment_Field"
                && property.name() == "GUID"
            {
                continue;
            }
            let index = names
                .iter()
                .position(|name| *name == property.name())
                .unwrap();
            body.extend(value(
                index,
                property.native_type(),
                property.flags(),
                property.raw_value(),
            ));
        }
        let kind = match group.kind() {
            PropertyGroupKind::Table => {
                body.extend(value(map_index, 11, 0, &literal_map(stale)));
                0
            }
            PropertyGroupKind::Column => 1,
            _ => panic!("unexpected source property scope"),
        };
        result.extend(block(kind, &body));
    }
    result
}

pub(super) fn source(missing_attachment_guid: bool, stale: bool) -> model::Case {
    let mut case = catalog_flag::source();
    let properties = bag(&case.before, missing_attachment_guid, stale);
    let page = &case.before[4 * 4096..5 * 4096];
    let mut rows = Vec::new();
    for slot in 0..6 {
        let start =
            u16::from_le_bytes(page[14 + slot * 2..16 + slot * 2].try_into().unwrap()) as usize;
        let end = if slot == 0 {
            4096
        } else {
            u16::from_le_bytes(page[12 + slot * 2..14 + slot * 2].try_into().unwrap()) as usize
        };
        let mut row = page[start..end].to_vec();
        if slot == 2 {
            let property_start =
                u16::from_le_bytes(row[row.len() - 7..row.len() - 5].try_into().unwrap());
            row.truncate(property_start as usize);
            // A fixed two-chunk catalog LVAL prevents this larger authored
            // property bag from depending on spare catalog DATA capacity.
            row.extend((properties.len() as u32).to_le_bytes());
            row.extend((23_u32 << 8).to_le_bytes());
            row.extend([0; 4]);
            row.extend((row.len() as u16).to_le_bytes());
            row.extend(property_start.to_le_bytes());
            row.extend(16_u16.to_le_bytes());
            row.extend(2_u16.to_le_bytes());
            row.push(0x3f);
        }
        rows.push((0, row));
    }
    let payload: usize = rows.iter().map(|(_, row)| row.len()).sum();
    let mut page = support::data_page(2, &rows);
    support::put16(&mut page, 2, (4096 - 14 - rows.len() * 2 - payload) as u16);
    assert!(properties.len() > 2048);
    assert!(properties.len() - 2048 <= 4096 - 20);
    let mut first = (31_u32 << 8).to_le_bytes().to_vec();
    first.extend(&properties[..2048]);
    let mut last = 0_u32.to_le_bytes().to_vec();
    last.extend(&properties[2048..]);
    for bytes in [&mut case.before, &mut case.expected] {
        bytes[4 * 4096..5 * 4096].copy_from_slice(&page);
        for (number, body) in [(23, &first), (31, &last)] {
            let mut page = support::data_page(0, &[(0, body.clone())]);
            page[4..8].copy_from_slice(b"LVAL");
            support::put16(&mut page, 2, (4096 - 16 - body.len()) as u16);
            bytes[number * 4096..(number + 1) * 4096].copy_from_slice(&page);
        }
    }
    case
}

#[test]
fn qualify_preserved_boolean_literal_name_map_sources_before_validator_or_writer() {
    for (missing, stale) in [(false, false), (true, false), (false, true)] {
        let case = source(missing, stale);
        assert_eq!(
            case.before
                .iter()
                .zip(&case.expected)
                .enumerate()
                .filter_map(|(offset, (before, after))| (before != after)
                    .then_some((offset, before ^ after)))
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
            let table = properties.group(PropertyGroupKind::Table, "").unwrap();
            assert_eq!(
                table.get("NameMap").unwrap().raw_value(),
                literal_map(stale)
            );
            let attachment = properties
                .group(PropertyGroupKind::Column, "Attachment_Field")
                .unwrap();
            assert_eq!(attachment.get("GUID").is_none(), missing);
            let schema = inspect_table_schema(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(schema.columns().len(), 16);
            let links = read_attachment_column(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                "Attachment_Field",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(links.links().parents()[0].child_count(), 2);
            let cached = read_stored_int32_calculation(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                "Calculated",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(cached.values(), &[Some(2), Some(4), Some(6)]);
        }
    }
}

#[test]
#[ignore = "Explicit one-shot private source export for an inert independent reader"]
fn export_preserved_name_map_sources_for_independent_reader() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(".cache/ace16-preserved-scalar-update-next-20261030/name-map-sources");
    std::fs::create_dir_all(&directory).unwrap();
    for (id, missing, stale) in [
        ("complete", false, false),
        ("missing-attachment-guid", true, false),
        ("stale", false, true),
    ] {
        let case = source(missing, stale);
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(directory.join(format!("{id}.accdb")))
            .unwrap();
        file.write_all(&case.before).unwrap();
        file.sync_all().unwrap();

        let mut map = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(directory.join(format!("{id}.bin")))
            .unwrap();
        map.write_all(&literal_map(stale)).unwrap();
        map.sync_all().unwrap();
    }
}
