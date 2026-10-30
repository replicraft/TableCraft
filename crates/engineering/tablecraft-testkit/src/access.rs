//! Literal Jet/ACE test images, without production codec dependencies.

/// Write a literal little-endian test word into an existing byte buffer.
pub fn put16(b: &mut [u8], at: usize, value: u16) {
    b[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
/// Write a literal little-endian test integer into an existing byte buffer.
pub fn put32(b: &mut [u8], at: usize, value: u32) {
    b[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
/// Build an independent 4 KiB DATA page with explicit slot flags and bodies.
pub fn data_page(owner: u32, rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut page = vec![0; 4096];
    page[0] = 1;
    page[1] = 1;
    put32(&mut page, 4, owner);
    put16(&mut page, 12, rows.len() as u16);
    let mut end = 4096;
    for (i, (flags, row)) in rows.iter().enumerate() {
        let start = end - row.len();
        page[start..end].copy_from_slice(row);
        put16(&mut page, 14 + 2 * i, (start as u16) | flags);
        end = start;
    }
    page
}
/// Encode the five-field catalog record used by the raw fixture images.
pub fn record(id: i32, parent: i32, kind: i16, flags: u32, name: &[u8]) -> Vec<u8> {
    let mut row = vec![0; 16];
    put16(&mut row, 0, 5);
    put32(&mut row, 2, id as u32);
    put32(&mut row, 6, parent as u32);
    put16(&mut row, 10, kind as u16);
    put32(&mut row, 12, flags);
    row.extend_from_slice(name);
    let end = row.len() as u16;
    row.extend_from_slice(&end.to_le_bytes());
    row.extend_from_slice(&16_u16.to_le_bytes());
    row.extend_from_slice(&1_u16.to_le_bytes());
    row.push(0x1f);
    row
}
/// Build a small catalog fixture with optional forwarding and indirect allocation.
pub fn image(overflow: bool, indirect: bool) -> Vec<u8> {
    let mut bytes = vec![0; 7 * 4096];
    bytes[..4].copy_from_slice(&[0, 1, 0, 0]);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 2;
    bytes[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
    let mut def = vec![0; 4096];
    def[0] = 2;
    def[1] = 1;
    put32(&mut def, 8, 4096);
    put32(&mut def, 16, 2);
    put16(&mut def, 41, 5);
    put16(&mut def, 43, 1);
    put16(&mut def, 45, 5);
    put32(&mut def, 55, 3 << 8);
    let fields = [
        ("Id", 4, 0, 0, 4),
        ("ParentId", 4, 1, 4, 4),
        ("Name", 10, 2, 0, 510),
        ("Type", 3, 3, 8, 2),
        ("Flags", 4, 4, 10, 4),
    ];
    let mut names = 63 + fields.len() * 25;
    for (i, (name, kind, number, offset, length)) in fields.iter().enumerate() {
        let at = 63 + i * 25;
        def[at] = *kind;
        put16(&mut def, at + 5, *number);
        def[at + 15] = u8::from(*kind != 10);
        put16(&mut def, at + 21, *offset);
        put16(&mut def, at + 23, *length);
        let text = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        put16(&mut def, names, text.len() as u16);
        names += 2;
        def[names..names + text.len()].copy_from_slice(&text);
        names += text.len();
    }
    bytes[2 * 4096..3 * 4096].copy_from_slice(&def);
    let table = record(100, 0, 1, 0, b"\xff\xfeTable");
    let form = record(-1, 100, -32768, 0, b"\xff\xfeForm");
    let rows = if overflow {
        vec![(0, table), (0x4000, (5_u32 << 8).to_le_bytes().to_vec())]
    } else {
        vec![(0, table), (0, form.clone())]
    };
    bytes[4 * 4096..5 * 4096].copy_from_slice(&data_page(2, &rows));
    if overflow {
        bytes[5 * 4096..6 * 4096].copy_from_slice(&data_page(2, &[(0x8000, form)]));
    }
    let bits = if overflow { 0x30 } else { 0x10 };
    let map = if indirect {
        let mut p = vec![0; 4096];
        p[..4].copy_from_slice(&[5, 1, 0, 0]);
        p[4] = bits;
        bytes[6 * 4096..7 * 4096].copy_from_slice(&p);
        vec![1, 6, 0, 0, 0]
    } else {
        vec![0, 0, 0, 0, 0, bits]
    };
    bytes[3 * 4096..4 * 4096].copy_from_slice(&data_page(1, &[(0, map)]));
    bytes
}

/// Literal ordinary INSERT seeds without native engine dependencies.
pub mod insert_seed;
/// Literal definition framing used by source models.
pub mod map_growth;
/// Shared literal populations and KEY/ROW framing for two physical owners.
pub mod multiple_indexes;
/// Independent original mutation property/definition builders.
pub mod mutation_fixture;
/// Complete independently authored secondary-index creation images.
pub mod secondary_index;
