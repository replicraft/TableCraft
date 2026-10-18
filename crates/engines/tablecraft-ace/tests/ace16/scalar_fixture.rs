use super::{fixture, support};

// Independent golden definition/row bytes; this is a synthetic CI input, not
// an ACE-authored file. The generated Java fixtures supply separate evidence.
pub fn fixed_scalar(code: u8, kind: u8, width: usize, values: &[Option<Vec<u8>>]) -> Vec<u8> {
    let mut bytes = fixture::with_rule("Caption", "modern");
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    bytes[10 * 4096 + 63 + 25] = kind;
    support::put16(&mut bytes, 10 * 4096 + 63 + 25 + 23, width as u16);
    support::put32(&mut bytes, 10 * 4096 + 16, values.len() as u32);
    let rows = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend_from_slice(&(index as i32 + 1).to_le_bytes());
            match value {
                Some(value) => {
                    assert_eq!(value.len(), width);
                    row.extend_from_slice(value);
                }
                None => row.resize(row.len() + width, 0),
            }
            row.push(if value.is_some() { 3 } else { 1 });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &rows));
    bytes
}
