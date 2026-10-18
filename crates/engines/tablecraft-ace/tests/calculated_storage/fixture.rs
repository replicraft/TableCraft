use super::{fixture, support};

// Original synthetic format bytes. Uses a real inline MR2 property carrier from
// the shared golden builder; no Java or third-party binary fixture is required.
pub fn database(values: &[Option<i32>]) -> Vec<u8> {
    let mut bytes = fixture::with_rule("Expression", "[Id]*2");
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    let start =
        u16::from_le_bytes(bytes[4 * 4096 + 18..4 * 4096 + 20].try_into().unwrap()) as usize;
    let mut item = bytes[4 * 4096 + start..4 * 4096 + start + 23].to_vec();
    let name = |text: &str| {
        let data = text
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        [(data.len() as u16).to_le_bytes().as_slice(), &data].concat()
    };
    let block = |kind: u16, body: &[u8]| {
        [
            ((body.len() + 6) as u32).to_le_bytes().as_slice(),
            &kind.to_le_bytes(),
            body,
        ]
        .concat()
    };
    let expression = "[Id]*2"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let expression = [
        ((expression.len() + 8) as u16).to_le_bytes().as_slice(),
        &[0, 10, 0, 0],
        &(expression.len() as u16).to_le_bytes(),
        &expression,
    ]
    .concat();
    let label = name("Score");
    let group = [
        ((label.len() + 4) as u32).to_le_bytes().as_slice(),
        &label,
        &expression,
        &[9, 0, 0, 2, 1, 0, 1, 0, 4],
    ]
    .concat();
    let properties = [
        b"MR2\0".as_slice(),
        &block(0x80, &[name("Expression"), name("ResultType")].concat()),
        &block(1, &group),
    ]
    .concat();
    item.extend((0x80000000 | properties.len() as u32).to_le_bytes());
    item.extend([0; 8]);
    item.extend(properties);
    item.extend((item.len() as u16).to_le_bytes());
    item.extend(23_u16.to_le_bytes());
    item.extend(16_u16.to_le_bytes());
    item.extend(2_u16.to_le_bytes());
    item.push(0x3f);
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
    let field = 10 * 4096 + 63 + 25;
    bytes[field + 15] = 2;
    bytes[field + 16] = 0xc0;
    support::put16(&mut bytes, field + 23, 39);
    support::put16(&mut bytes, 10 * 4096 + 43, 1);
    support::put32(&mut bytes, 10 * 4096 + 16, values.len() as u32);
    let rows = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend((index as i32 + 1).to_le_bytes());
            if let Some(value) = value {
                row.extend([0; 16]);
                row.extend(4_u32.to_le_bytes());
                row.extend(value.to_le_bytes());
                row.extend([0; 3]);
            }
            row.extend((row.len() as u16).to_le_bytes());
            row.extend(6_u16.to_le_bytes());
            row.extend(1_u16.to_le_bytes());
            row.push(if value.is_some() { 3 } else { 1 });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &rows));
    bytes
}
