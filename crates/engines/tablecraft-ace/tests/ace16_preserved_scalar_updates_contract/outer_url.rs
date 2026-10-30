//! Literal outer FileURL storage and ownership derivatives, before writer APIs.
use super::{SCOPE, budget, catalog_flag, model, modern_attachment, support};
use std::io::Cursor;
use tablecraft_ace::{inspect_table_schema, read_attachment_column};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const URL: &str = "relative?external";

fn bitmap(members: &[usize]) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    for &number in members {
        bytes[5 + number / 8] |= 1 << (number % 8);
    }
    bytes
}

fn install_page(bytes: &mut [u8], number: usize, owner: u32, rows: &[(u16, Vec<u8>)]) {
    let mut page = support::data_page(owner, rows);
    let payload: usize = rows.iter().map(|(_, row)| row.len()).sum();
    support::put16(&mut page, 2, (PAGE - 14 - rows.len() * 2 - payload) as u16);
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(&page);
}

pub(super) fn source(external: bool, overlapping_owned: bool) -> model::Case {
    let mut case = catalog_flag::source();
    if !external {
        assert!(!overlapping_owned);
        return case;
    }

    let mut rows = (0..3)
        .map(|slot| {
            let (start, end) = modern_attachment::span(&case.before, slot);
            case.before[start..end].to_vec()
        })
        .collect::<Vec<_>>();
    let row = &rows[0];
    // Six variable fields, seven reverse offsets, count word and one mask byte.
    let footer = row.len() - 17;
    let mut offsets = (0..7)
        .map(|index| {
            let at = footer + (6 - index) * 2;
            u16::from_le_bytes(row[at..at + 2].try_into().unwrap()) as usize
        })
        .collect::<Vec<_>>();
    let payload = URL
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut descriptor = (0x4000_0000_u32 | payload.len() as u32)
        .to_le_bytes()
        .to_vec();
    descriptor.extend((23_u32 << 8).to_le_bytes());
    descriptor.extend([0; 4]);
    let old_length = offsets[5] - offsets[4];
    let mut updated = row[..offsets[4]].to_vec();
    updated.extend(&descriptor);
    updated.extend(&row[offsets[5]..footer]);
    for offset in &mut offsets[5..] {
        *offset = *offset - old_length + descriptor.len();
    }
    for offset in offsets.into_iter().rev() {
        updated.extend((offset as u16).to_le_bytes());
    }
    updated.extend(6_u16.to_le_bytes());
    updated.push(0xff);
    rows[0] = updated;

    let child = &case.before[20 * PAGE..21 * PAGE];
    let mut tail = 63 + 8 * 25;
    for _ in 0..8 {
        let length = u16::from_le_bytes(child[tail..tail + 2].try_into().unwrap()) as usize;
        tail += 2 + length;
    }
    assert_eq!(
        u16::from_le_bytes(child[tail..tail + 2].try_into().unwrap()),
        0xffff
    );
    let owned = if overlapping_owned {
        vec![12, 23]
    } else {
        vec![23]
    };
    let maps = [vec![22], vec![22], owned, vec![23]]
        .iter()
        .map(|members| (0, bitmap(members)))
        .collect::<Vec<_>>();
    let rows = rows.into_iter().map(|row| (0, row)).collect::<Vec<_>>();
    for bytes in [&mut case.before, &mut case.expected] {
        install_page(bytes, 22, 20, &rows);
        install_page(bytes, 23, 0, &[(0, payload.clone())]);
        bytes[23 * PAGE + 4..23 * PAGE + 8].copy_from_slice(b"LVAL");
        install_page(bytes, 21, 0, &maps);
        support::put16(bytes, 20 * PAGE + tail, 6);
        support::put32(bytes, 20 * PAGE + tail + 2, (21 << 8) | 2);
        support::put32(bytes, 20 * PAGE + tail + 6, (21 << 8) | 3);
        support::put16(bytes, 20 * PAGE + tail + 10, 0xffff);
    }
    case
}

#[test]
fn qualify_preserved_boolean_inline_external_and_overlapping_file_url_sources_before_writer() {
    for (external, overlapping) in [(false, false), (true, false), (true, true)] {
        let case = source(external, overlapping);
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
            let schema = inspect_table_schema(
                &mut Cursor::new(bytes),
                SCOPE,
                "Children",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let url = &schema.columns()[6];
            assert_eq!(
                (
                    url.native_type(),
                    url.flags(),
                    url.extended_flags(),
                    url.storage_length()
                ),
                (12, 2, 0x10, 0)
            );
            let attachments = read_attachment_column(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                "Attachment_Field",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                attachments.stored_value(0, 0, "FileURL"),
                Some(&Value::Text(
                    if external { URL } else { "relative?x" }.into()
                ))
            );
            assert_eq!(
                attachments.stored_value(0, 1, "FileURL"),
                Some(&Value::Null)
            );
            assert_eq!(
                attachments.stored_value(1, 0, "FileURL"),
                Some(&Value::Text(String::new()))
            );
        }
    }
}
