use super::{base, support};

pub fn inline(bytes: &[u8]) -> Vec<u8> {
    let mut output = (0x8000_0000 | bytes.len() as u32).to_le_bytes().to_vec();
    output.extend([0; 8]);
    output.extend(bytes);
    output
}
pub fn external(bytes: &[u8]) -> Vec<u8> {
    let mut output = (0x4000_0000 | bytes.len() as u32).to_le_bytes().to_vec();
    output.extend((13_u32 << 8).to_le_bytes());
    output.extend([0; 4]);
    output
}
pub fn database(code: u8, payloads: &[Option<Vec<u8>>; 3]) -> Vec<u8> {
    let mut bytes = base::database();
    bytes[20] = code;
    if code != 1 {
        bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    }
    let mut item = vec![0; 16];
    support::put16(&mut item, 0, 9);
    support::put32(&mut item, 2, 10);
    support::put16(&mut item, 10, (-32761_i16) as u16);
    let mut offsets = vec![16_u16];
    item.extend(b"\xff\xfeItems");
    offsets.push(item.len() as u16);
    offsets.push(item.len() as u16);
    // Native order is body, extra, module; the public result uses body/module/extra.
    for position in [0, 2, 1] {
        if let Some(payload) = &payloads[position] {
            item.extend(payload);
        }
        offsets.push(item.len() as u16);
    }
    for offset in offsets.into_iter().rev() {
        item.extend(offset.to_le_bytes());
    }
    item.extend(5_u16.to_le_bytes());
    item.push(
        0x1f | if payloads[0].is_some() { 0x40 } else { 0 }
            | if payloads[2].is_some() { 0x80 } else { 0 },
    );
    item.push(u8::from(payloads[1].is_some()));
    bytes[4 * 4096..5 * 4096].copy_from_slice(&support::data_page(
        2,
        &[
            (
                0,
                support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
            ),
            (
                0,
                support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
            ),
            (0, item),
        ],
    ));
    bytes
}
pub fn lval(bytes: &[u8]) -> Vec<u8> {
    let mut page = support::data_page(0, &[(0, bytes.to_vec())]);
    page[4..8].copy_from_slice(b"LVAL");
    page
}
