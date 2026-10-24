use super::*;

pub fn row(id: i32) -> Vec<u8> {
    let mut row = 255_u16.to_le_bytes().to_vec();
    row.extend(id.to_le_bytes());
    row.extend(0..254_u8);
    for offset in (6..=260_u16).rev() {
        row.extend(offset.to_le_bytes());
    }
    row.extend(254_u16.to_le_bytes());
    row.extend([0xff; 32]);
    for field in [7, 8, 127, 128, 253] {
        row[772 + field / 8] &= !(1 << (field % 8));
    }
    assert_eq!(row.len(), 804);
    row
}

pub fn expected(id: i32, projection: Option<&[&str]>) -> Vec<Value> {
    match projection {
        Some([]) => return Vec::new(),
        Some(["ID"]) => return vec![Value::Int32(id)],
        _ => {}
    }
    let mut values = vec![Value::Int32(id)];
    for field in 1..255 {
        values.push(if [7, 8, 127, 128, 253].contains(&field) {
            Value::Null
        } else {
            Value::Binary(vec![(field - 1) as u8])
        });
    }
    values
}

// Independent golden fields and physical page words; no production constants.
pub fn database(code: u8, damage: Option<(usize, u16)>) -> Vec<u8> {
    let mut bytes = base::database();
    if code != 1 {
        bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    }
    bytes[20] = code;

    let mut definition = vec![0; 63 + 255 * 25];
    definition[..2].copy_from_slice(&[2, 1]);
    put32(&mut definition, 4, 14);
    put32(&mut definition, 16, 3);
    put16(&mut definition, 41, 255);
    put16(&mut definition, 43, 254);
    put16(&mut definition, 45, 255);
    put32(&mut definition, 55, 11 << 8);
    for field in 0..255 {
        let entry = 63 + field * 25;
        definition[entry] = if field == 0 { 4 } else { 9 };
        put16(&mut definition, entry + 5, field as u16);
        if field == 0 {
            definition[entry + 15] = 3;
            put16(&mut definition, entry + 23, 4);
        } else {
            put16(&mut definition, entry + 7, (field - 1) as u16);
            put16(&mut definition, entry + 23, 1);
        }
        let name = if field == 0 {
            "ID".into()
        } else {
            format!("F{field:03}")
        };
        let name: Vec<_> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        definition.extend((name.len() as u16).to_le_bytes());
        definition.extend(name);
    }
    let length = definition.len();
    put32(&mut definition, 8, length as u32);
    bytes[10 * PAGE_BYTES..11 * PAGE_BYTES].copy_from_slice(&definition[..PAGE_BYTES]);
    let chunks = definition[PAGE_BYTES..].chunks(4088);
    let count = chunks.len();
    for (index, chunk) in chunks.enumerate() {
        let mut page = vec![0; PAGE_BYTES];
        page[..2].copy_from_slice(&[2, 1]);
        if index + 1 < count {
            put32(&mut page, 4, (15 + index) as u32);
        }
        page[8..8 + chunk.len()].copy_from_slice(chunk);
        bytes.extend(page);
    }

    let mut last = row(3);
    if let Some((offset, value)) = damage {
        put16(&mut last, offset, value);
    }
    bytes[12 * PAGE_BYTES..13 * PAGE_BYTES]
        .copy_from_slice(&data_page(10, &[(0, row(1)), (0, row(2)), (0, last)]));
    bytes
}
