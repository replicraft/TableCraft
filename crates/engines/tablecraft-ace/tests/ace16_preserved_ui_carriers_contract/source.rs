//! Independently framed UI-carrier derivatives; no preservation writer executes.
use super::{SCOPE, budget, model, name_map, support};
use std::io::Cursor;
use tablecraft_ace::{
    PropertyGroupKind, inspect_table_properties, read_attachment_column,
    read_stored_int32_calculation, read_table_projection,
};
use tablecraft_core::values::Value;

#[derive(Clone, Copy)]
pub(super) enum Carrier {
    WideSigned,
    WidePositive,
    UnqualifiedUpperWord,
    ThreeBytes,
    WrongNativeKind,
    DdlFlag,
}

fn short(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

fn long(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

fn transform(original: &[u8], carrier: Carrier) -> Vec<u8> {
    assert_eq!(&original[..4], b"MR2\0");
    let names_end = 4 + long(original, 4);
    assert_eq!(short(original, 8), 0x80);
    let mut cursor = 10;
    let mut names = Vec::new();
    while cursor < names_end {
        let length = short(original, cursor);
        cursor += 2;
        let units = original[cursor..cursor + length]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect::<Vec<_>>();
        names.push(String::from_utf16(&units).unwrap());
        cursor += length;
    }
    assert_eq!(cursor, names_end);
    let mut result = original[..names_end].to_vec();
    let mut changed = 0;
    while cursor < original.len() {
        let end = cursor + long(original, cursor);
        let kind = short(original, cursor + 4);
        assert!(kind <= 1);
        let start = cursor + 6;
        let properties = start + long(original, start);
        let mut body = original[start..properties].to_vec();
        cursor = properties;
        while cursor < end {
            let record_end = cursor + short(original, cursor);
            let flags = original[cursor + 2];
            let native = original[cursor + 3];
            let name = short(original, cursor + 4);
            let size = short(original, cursor + 6);
            assert_eq!(record_end, cursor + 8 + size);
            if matches!(names[name].as_str(), "ColumnWidth" | "ColumnOrder") {
                assert_eq!((kind, flags, native, size), (1, 0, 3, 2));
                let original_value =
                    i16::from_le_bytes(original[cursor + 8..record_end].try_into().unwrap());
                let value =
                    if matches!(carrier, Carrier::WidePositive) && names[name] == "ColumnWidth" {
                        1440
                    } else {
                        i32::from(original_value)
                    };
                let mut raw = value.to_le_bytes().to_vec();
                if matches!(carrier, Carrier::UnqualifiedUpperWord) && names[name] == "ColumnWidth"
                {
                    // The signed low word is -1, but the extra high word is0.
                    // This is an unqualified carrier, not a malformed reader claim.
                    raw[2..].fill(0);
                }
                if matches!(carrier, Carrier::ThreeBytes) {
                    raw.truncate(3);
                }
                body.extend(((8 + raw.len()) as u16).to_le_bytes());
                body.push(if matches!(carrier, Carrier::DdlFlag) {
                    1
                } else {
                    0
                });
                body.push(if matches!(carrier, Carrier::WrongNativeKind) {
                    4
                } else {
                    3
                });
                body.extend((name as u16).to_le_bytes());
                body.extend((raw.len() as u16).to_le_bytes());
                body.extend(raw);
                changed += 1;
            } else {
                body.extend(&original[cursor..record_end]);
            }
            cursor = record_end;
        }
        assert_eq!(cursor, end);
        result.extend(((body.len() + 6) as u32).to_le_bytes());
        result.extend((kind as u16).to_le_bytes());
        result.extend(body);
    }
    assert_eq!(changed, 32);
    result
}

pub(super) fn source(carrier: Carrier) -> model::Case {
    let mut case = name_map::source(false, false);
    let properties = inspect_table_properties(
        &mut Cursor::new(&case.before),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let bag = transform(properties.raw_bytes(), carrier);
    assert!(bag.len() > 2048 && bag.len() - 2048 <= 4076);
    for bytes in [&mut case.before, &mut case.expected] {
        // Update only the synthetic catalog's same-size chain descriptor length.
        let row = short(bytes, 4 * 4096 + 18);
        let previous_end = short(bytes, 4 * 4096 + 16);
        let property_start = short(bytes, 4 * 4096 + previous_end - 7);
        let descriptor = 4 * 4096 + row + property_start;
        bytes[descriptor..descriptor + 4].copy_from_slice(&(bag.len() as u32).to_le_bytes());
        for (page, next, data) in [(23, 31, &bag[..2048]), (31, 0, &bag[2048..])] {
            let mut body = ((next as u32) << 8).to_le_bytes().to_vec();
            body.extend(data);
            let mut framing = support::data_page(0, &[(0, body.clone())]);
            framing[4..8].copy_from_slice(b"LVAL");
            support::put16(&mut framing, 2, (4096 - 16 - body.len()) as u16);
            bytes[page * 4096..(page + 1) * 4096].copy_from_slice(&framing);
        }
    }
    case
}

#[test]
fn qualify_preserved_wide_ui_carrier_sources_before_admission_changes() {
    for carrier in [
        Carrier::WideSigned,
        Carrier::WidePositive,
        Carrier::UnqualifiedUpperWord,
        Carrier::ThreeBytes,
        Carrier::WrongNativeKind,
        Carrier::DdlFlag,
    ] {
        let case = source(carrier);
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
        for (bytes, first) in [(&case.before, false), (&case.expected, true)] {
            let properties = inspect_table_properties(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let id = properties.group(PropertyGroupKind::Column, "ID").unwrap();
            let width = id.get("ColumnWidth").unwrap();
            let order = id.get("ColumnOrder").unwrap();
            assert_eq!(
                order.raw_value(),
                if matches!(carrier, Carrier::ThreeBytes) {
                    &[0, 0, 0][..]
                } else {
                    &[0, 0, 0, 0][..]
                }
            );
            let expected_width = match carrier {
                Carrier::WidePositive => vec![0xa0, 0x05, 0, 0],
                Carrier::UnqualifiedUpperWord => vec![0xff, 0xff, 0, 0],
                Carrier::ThreeBytes => vec![0xff, 0xff, 0xff],
                _ => vec![0xff; 4],
            };
            assert_eq!(width.raw_value(), expected_width);
            assert_eq!(
                width.native_type(),
                if matches!(carrier, Carrier::WrongNativeKind) {
                    4
                } else {
                    3
                }
            );
            assert_eq!(width.flags(), u8::from(matches!(carrier, Carrier::DdlFlag)));
            let rows = read_table_projection(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                &["ID", "Yes_Field"],
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows()[0], [Value::Int32(1), Value::Boolean(first)]);
            let attachments = read_attachment_column(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                "Attachment_Field",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(attachments.links().parents()[0].child_count(), 2);
            let calculation = read_stored_int32_calculation(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                "Calculated",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(calculation.values(), &[Some(2), Some(4), Some(6)]);
        }
    }
}
