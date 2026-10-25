use super::support::{data_page, image, put16, put32};

const PAGE_BYTES: usize = 4096;
const FIELD_START: usize = 63;
const FIELD_BYTES: usize = 25;

pub fn database(
    kind: i16,
    database: Option<&str>,
    table: Option<&str>,
    connect: Option<&str>,
) -> Vec<u8> {
    let mut bytes = image(false, false);
    let mut definition = vec![0; PAGE_BYTES];
    definition[..2].copy_from_slice(&[2, 1]);
    put32(&mut definition, 8, PAGE_BYTES as u32);
    put32(&mut definition, 16, 2);
    put16(&mut definition, 41, 8);
    put16(&mut definition, 43, 4);
    put16(&mut definition, 45, 8);
    put32(&mut definition, 55, 3 << 8);
    let fields = [
        ("Id", 4, 0, 4),
        ("ParentId", 4, 4, 4),
        ("Name", 10, 0, 510),
        ("Type", 3, 8, 2),
        ("Flags", 4, 10, 4),
        ("Database", 12, 0, 0),
        ("Connect", 12, 0, 0),
        ("ForeignName", 10, 0, 510),
    ];
    let mut names = FIELD_START + fields.len() * FIELD_BYTES;
    let mut variable = 0;
    for (number, (name, kind, offset, length)) in fields.into_iter().enumerate() {
        let at = FIELD_START + number * FIELD_BYTES;
        definition[at] = kind;
        put16(&mut definition, at + 5, number as u16);
        if matches!(kind, 10 | 12) {
            definition[at + 15] = 0x12;
            put16(&mut definition, at + 7, variable);
            variable += 1;
        } else {
            definition[at + 15] = 0x13;
        }
        put16(&mut definition, at + 21, offset);
        put16(&mut definition, at + 23, length);
        let encoded = utf16(name);
        put16(&mut definition, names, encoded.len() as u16);
        names += 2;
        definition[names..names + encoded.len()].copy_from_slice(&encoded);
        names += encoded.len();
    }
    bytes[2 * PAGE_BYTES..3 * PAGE_BYTES].copy_from_slice(&definition);
    let rows = [
        (0, row(2, 0, 1, "MSysObjects", None, None, None)),
        (0, row(-1, 0, kind, "Linked", database, table, connect)),
    ];
    bytes[4 * PAGE_BYTES..5 * PAGE_BYTES].copy_from_slice(&data_page(2, &rows));
    bytes
}

pub fn with_parent(parent: i32) -> Vec<u8> {
    let mut bytes = database(6, Some("missing"), Some("Remote"), None);
    let rows = [
        (0, row(2, 0, 1, "MSysObjects", None, None, None)),
        (
            0,
            row(
                -1,
                parent,
                6,
                "Linked",
                Some("missing"),
                Some("Remote"),
                None,
            ),
        ),
    ];
    bytes[4 * PAGE_BYTES..5 * PAGE_BYTES].copy_from_slice(&data_page(2, &rows));
    bytes
}

pub fn with_duplicate_id() -> Vec<u8> {
    let mut bytes = database(6, Some("missing"), Some("Remote"), None);
    let rows = [
        (0, row(2, 0, 1, "MSysObjects", None, None, None)),
        (
            0,
            row(2, 0, 6, "Linked", Some("missing"), Some("Remote"), None),
        ),
    ];
    bytes[4 * PAGE_BYTES..5 * PAGE_BYTES].copy_from_slice(&data_page(2, &rows));
    bytes
}

pub fn rename_database(bytes: &mut [u8]) {
    let schema = &mut bytes[2 * PAGE_BYTES..3 * PAGE_BYTES];
    let needle = utf16("Database");
    let at = schema
        .windows(needle.len())
        .position(|part| part == needle)
        .unwrap();
    schema[at..at + needle.len()].copy_from_slice(&utf16("Databaze"));
}

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn memo(text: &str) -> Vec<u8> {
    let text = utf16(text);
    let mut value = (0x80000000 | text.len() as u32).to_le_bytes().to_vec();
    value.extend_from_slice(&[0; 8]);
    value.extend(text);
    value
}

fn row(
    id: i32,
    parent: i32,
    kind: i16,
    name: &str,
    database: Option<&str>,
    table: Option<&str>,
    connect: Option<&str>,
) -> Vec<u8> {
    let mut row = vec![0; 16];
    put16(&mut row, 0, 8);
    put32(&mut row, 2, id as u32);
    put32(&mut row, 6, parent as u32);
    put16(&mut row, 10, kind as u16);
    let fields = [
        Some(utf16(name)),
        database.map(memo),
        connect.map(memo),
        table.map(utf16),
    ];
    let mut offsets = vec![row.len() as u16];
    let mut mask = 0x1f;
    for (index, field) in fields.into_iter().enumerate() {
        if let Some(field) = field {
            row.extend(field);
            mask |= 1 << [2, 5, 6, 7][index];
        }
        offsets.push(row.len() as u16);
    }
    for offset in offsets.into_iter().rev() {
        row.extend_from_slice(&offset.to_le_bytes());
    }
    row.extend_from_slice(&4_u16.to_le_bytes());
    row.push(mask);
    row
}

pub fn set_column_byte(bytes: &mut [u8], column: usize, offset: usize, value: u8) {
    bytes[2 * PAGE_BYTES + FIELD_START + column * FIELD_BYTES + offset] = value;
}

pub fn malformed_database() -> Vec<u8> {
    let mut bytes = database(6, Some("missing"), Some("Remote"), None);
    let encoded = memo("missing");
    let offset = bytes
        .windows(encoded.len())
        .position(|part| part == encoded)
        .unwrap();
    bytes[offset] = 0xff;
    bytes
}
