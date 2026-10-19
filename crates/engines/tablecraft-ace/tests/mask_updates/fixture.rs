use super::{
    base,
    support::{data_page, put32},
};

// Independent synthetic fields/row bytes. Boolean slots span two mask bytes.
pub fn database(code: u8, nullable: &[bool]) -> Vec<u8> {
    let fields = [
        ("First", 1, 0, 1),
        ("Tiny", 2, 0, 1),
        ("Number", 4, 1, 4),
        ("Large", 0x13, 5, 8),
        ("Guid", 15, 13, 16),
        ("Other", 2, 29, 1),
        ("Guard", 1, 0, 1),
        ("Seventh", 1, 0, 1),
        ("Eighth", 1, 0, 1),
        ("Last", 1, 0, 1),
    ];
    let mut bytes = base::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    bytes[10 * 4096..11 * 4096].copy_from_slice(&base::definition(
        &fields,
        nullable.len() as u32,
        11 << 8,
    ));
    let rows = nullable
        .iter()
        .map(|missing| {
            let mut row = 10_u16.to_le_bytes().to_vec();
            row.push(11);
            row.extend(20_i32.to_le_bytes());
            row.extend(7_i64.to_le_bytes());
            row.extend([0x11; 16]);
            row.push(31);
            row.extend([if *missing { 0xa1 } else { 0xbf }, 0x02]);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&data_page(10, &rows));
    put32(&mut bytes, 10 * 4096 + 16, nullable.len() as u32);
    bytes
}
