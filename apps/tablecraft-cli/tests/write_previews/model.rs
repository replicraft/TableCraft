//! Independent synthetic host inputs, using actual maintained golden owners.
use super::{fixture, index_fixture, support};

const PAGE: usize = 4096;

fn install(bytes: &mut [u8], number: usize, page: Vec<u8>) {
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(&page);
}

fn map(members: impl IntoIterator<Item = u32>) -> Vec<u8> {
    let mut row = vec![0; 69];
    for member in members {
        row[5 + member as usize / 8] |= 1 << (member % 8);
    }
    row
}

fn maps(bytes: &mut [u8], indexed: bool) {
    let eof = (bytes.len() / PAGE) as u32;
    install(bytes, 1, support::data_page(1, &[(0, map(eof..512))]));
    install(
        bytes,
        11,
        support::data_page(0, &[(0, map([12])), (0, map([12]))]),
    );
    support::put32(bytes, 10 * PAGE + 59, (11 << 8) | 1);
    if indexed {
        install(bytes, 14, support::data_page(0, &[(0, map([13]))]));
    }
}

fn complete_definition_tails(bytes: &mut [u8]) {
    // Independent literal grammar: physical counter blocks, field descriptors,
    // field names, physical/logical index descriptors, then logical index names.
    // The source factory intentionally predates strict column-map census and
    // leaves this trailer unspecified; establish complete source metadata here.
    for number in [2, 7, 10] {
        let page = &mut bytes[number * PAGE..(number + 1) * PAGE];
        let count16 = |offset| {
            usize::from(u16::from_le_bytes(
                page[offset..offset + 2].try_into().unwrap(),
            ))
        };
        let columns = count16(45);
        let physical = u32::from_le_bytes(page[51..55].try_into().unwrap()) as usize;
        let logical = u32::from_le_bytes(page[47..51].try_into().unwrap()) as usize;
        let mut tail = 63 + physical * 12 + columns * 25;
        for _ in 0..columns {
            tail += 2 + usize::from(u16::from_le_bytes(page[tail..tail + 2].try_into().unwrap()));
        }
        tail += physical * 52 + logical * 28;
        for _ in 0..logical {
            tail += 2 + usize::from(u16::from_le_bytes(page[tail..tail + 2].try_into().unwrap()));
        }
        assert_eq!(&page[tail..tail + 2], &[0, 0]);
        support::put16(page, tail, 0xffff);
        let length = tail + 2;
        support::put32(page, 8, length as u32);
        support::put16(page, 2, (PAGE - length - 8) as u16);
    }
}

pub(super) fn fixed(generated: bool) -> Vec<u8> {
    let mut bytes = index_fixture::database(false, false, false);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 5;
    // The source owner already encodes one ordinary required primary descriptor
    // on native column one (Score), proving that neither names nor ordinal zero
    // implicitly select a generated field.
    assert_eq!(bytes[10 * PAGE + 195 + 23], 1);
    if generated {
        bytes[10 * PAGE + 75 + 25 + 15] = 7;
        support::put32(&mut bytes, 10 * PAGE + 20, 30);
    }
    support::put16(&mut bytes, 12 * PAGE + 2, 4043);
    maps(&mut bytes, true);
    complete_definition_tails(&mut bytes);
    bytes
}

pub(super) fn variable(binary: bool) -> Vec<u8> {
    let mut bytes = fixture::with_rule("Caption", "host variable input");
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 5;
    let mut definition = fixture::definition(
        &[
            ("Id", 4, 0, 4),
            (
                "Score",
                if binary { 9 } else { 10 },
                0,
                if binary { 255 } else { 510 },
            ),
        ],
        3,
        11 << 8,
    );
    support::put16(&mut definition, 43, 1);
    support::put16(&mut definition, 63 + 25 + 7, 0);
    definition[63 + 25 + 15] = 2;
    install(&mut bytes, 10, definition);
    let mut records = Vec::new();
    for (ordinal, text) in ["Alice", "Bob", "null"].into_iter().enumerate() {
        let mut row = 2_u16.to_le_bytes().to_vec();
        row.extend((ordinal as i32 + 1).to_le_bytes());
        if binary {
            row.extend([ordinal as u8, 255]);
        } else {
            row.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        }
        let end = row.len() as u16;
        row.extend(end.to_le_bytes());
        row.extend(6_u16.to_le_bytes());
        row.extend(1_u16.to_le_bytes());
        row.push(3);
        records.push((0, row));
    }
    let used = records.iter().map(|(_, row)| row.len() + 2).sum::<usize>();
    let mut data = support::data_page(10, &records);
    support::put16(&mut data, 2, (PAGE - 14 - used) as u16);
    install(&mut bytes, 12, data);
    maps(&mut bytes, false);
    complete_definition_tails(&mut bytes);
    bytes
}
