use super::{calculated_fixture, support};

pub const PAGE: usize = 4096;

pub fn database(values: &[Option<i32>], descending: bool, aliases: bool) -> Vec<u8> {
    let mut bytes = calculated_fixture::database_with_expression(values, "1/0");
    bytes.resize(16 * PAGE, 0);
    let definition = &mut bytes[10 * PAGE..11 * PAGE];
    definition.copy_within(63..131, 75);
    definition[63..75].fill(0);
    support::put32(definition, 67, values.len() as u32);
    support::put32(definition, 47, if aliases { 2 } else { 1 });
    support::put32(definition, 51, 1);
    for component in 0..10 {
        support::put16(definition, 147 + component * 3, 0xffff);
    }
    support::put16(definition, 147, 0);
    definition[149] = u8::from(!descending);
    support::put32(definition, 177, 14 << 8);
    support::put32(definition, 181, 13);
    definition[189] = 0x81;
    if aliases {
        support::put32(definition, 227, 1);
    }
    let mut position = 195 + 28 * if aliases { 2 } else { 1 };
    for name in if aliases {
        vec!["IdOrder", "IdTwin"]
    } else {
        vec!["IdOrder"]
    } {
        let name: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        support::put16(definition, position, name.len() as u16);
        position += 2;
        definition[position..position + name.len()].copy_from_slice(&name);
        position += name.len();
    }
    bytes[14 * PAGE..15 * PAGE]
        .copy_from_slice(&support::data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 0x20])]));
    let leaf = &mut bytes[13 * PAGE..14 * PAGE];
    leaf.fill(0);
    leaf[..2].copy_from_slice(&[4, 1]);
    support::put32(leaf, 4, 10);
    let mut slots: Vec<usize> = (0..values.len()).collect();
    if descending {
        slots.reverse();
    }
    let mut active = 0;
    for slot in slots {
        let value = slot as i32 + 1;
        let mut key = vec![if descending { 0x80 } else { 0x7f }];
        key.extend(
            ((value as u32) ^ 0x80000000)
                .to_be_bytes()
                .map(|byte| if descending { !byte } else { byte }),
        );
        key.extend(((12_u32 << 8) | slot as u32).to_be_bytes());
        leaf[480 + active..480 + active + key.len()].copy_from_slice(&key);
        active += key.len();
        leaf[27 + active / 8] |= 1 << (active % 8);
    }
    support::put16(leaf, 2, (4096 - 480 - active) as u16);
    bytes
}

pub fn row_start(bytes: &[u8], slot: usize) -> usize {
    let at = 12 * PAGE + 14 + slot * 2;
    12 * PAGE + u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

pub fn forwarded(mut bytes: Vec<u8>) -> Vec<u8> {
    let rows: Vec<Vec<u8>> = (0..3)
        .map(|slot| {
            let start = row_start(&bytes, slot);
            let end = if slot == 0 {
                13 * PAGE
            } else {
                row_start(&bytes, slot - 1)
            };
            bytes[start..end].to_vec()
        })
        .collect();
    bytes[11 * PAGE + PAGE - 1] = 0x90;
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&support::data_page(
        10,
        &[
            (0x4000, (15_u32 << 8).to_le_bytes().to_vec()),
            (0, rows[1].clone()),
            (0, rows[2].clone()),
        ],
    ));
    bytes[15 * PAGE..16 * PAGE]
        .copy_from_slice(&support::data_page(10, &[(0x8000, rows[0].clone())]));
    bytes
}
