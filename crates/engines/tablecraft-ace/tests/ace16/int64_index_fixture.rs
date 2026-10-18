use super::{scalar_fixture, support};

// Independent synthetic format bytes, not an Access-authored compatibility claim.
pub fn database(code: u8, descending: bool, numbers: &[Option<i64>]) -> Vec<u8> {
    let payloads = numbers
        .iter()
        .map(|value| value.map(|number| number.to_le_bytes().to_vec()))
        .collect::<Vec<_>>();
    let mut bytes = scalar_fixture::fixed_scalar(code, 0x13, 8, &payloads);
    bytes.resize(15 * 4096, 0);
    let definition = &mut bytes[10 * 4096..11 * 4096];
    definition.copy_within(63..131, 75);
    definition[63..75].fill(0);
    support::put32(definition, 47, 1);
    support::put32(definition, 51, 1);
    for key in 0..10 {
        support::put16(definition, 143 + 4 + key * 3, 0xffff);
    }
    support::put16(definition, 147, 1);
    definition[149] = u8::from(!descending);
    support::put32(definition, 177, 13 << 8);
    support::put32(definition, 181, 14);
    definition[189] = 0x80;
    support::put16(definition, 223, 12);
    for (offset, byte) in "BigKey"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .enumerate()
    {
        definition[225 + offset] = byte;
    }
    bytes[13 * 4096..14 * 4096]
        .copy_from_slice(&support::data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 0x40])]));
    let mut entries = numbers.iter().copied().enumerate().collect::<Vec<_>>();
    entries.sort_by(|(a, x), (b, y)| {
        let order = x.cmp(y);
        (if descending { order.reverse() } else { order }).then(a.cmp(b))
    });
    let page = &mut bytes[14 * 4096..];
    page[..2].copy_from_slice(&[4, 1]);
    support::put32(page, 4, 10);
    let mut end = 0;
    for (slot, key) in entries {
        let mut encoded = vec![match (key, descending) {
            (None, false) => 0,
            (None, true) => 0xff,
            (Some(_), false) => 0x7f,
            (Some(_), true) => 0x80,
        }];
        if let Some(key) = key {
            let key = ((key as u64) ^ 0x8000_0000_0000_0000).to_be_bytes();
            encoded.extend(key.map(|byte| if descending { !byte } else { byte }));
        }
        encoded.extend(((12_u32 << 8) | slot as u32).to_be_bytes());
        page[480 + end..480 + end + encoded.len()].copy_from_slice(&encoded);
        end += encoded.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(page, 2, (4096 - 480 - end) as u16);
    bytes
}
