//! Original synthetic stored caches and ordinary key/ROW wires.
use super::{base, calculated_fixture, modern_fixture, support};

pub const PAGE: usize = 4096;
pub const ROOT: usize = 24;
pub const HIDDEN: usize = 26;
pub const IDS: [i32; 4] = [30, 10, 10, 20];
pub const CACHES: [Option<i32>; 4] = [Some(999), None, Some(41), Some(-7)];

pub fn plain(code: u8, descending: bool, forwarded: bool) -> Vec<u8> {
    let mut bytes = calculated_fixture::database_with_expression(&CACHES, "1/0");
    bytes[20] = code;
    for (slot, id) in IDS.iter().enumerate() {
        let start = row_start(&bytes, 12, slot);
        bytes[start + 2..start + 6].copy_from_slice(&id.to_le_bytes());
    }
    install_index(&mut bytes, descending);
    if forwarded {
        forward(&mut bytes);
    }
    bytes
}

pub fn row_start(bytes: &[u8], page: usize, slot: usize) -> usize {
    let at = page * PAGE + 14 + 2 * slot;
    page * PAGE + usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) & 0x1fff)
}

fn cache(value: i32) -> Vec<u8> {
    let mut wire = vec![0; 16];
    wire.extend(4_u32.to_le_bytes());
    wire.extend(value.to_le_bytes());
    wire.extend([0; 3]);
    wire
}

pub fn mixed(code: u8, descending: bool, forwarded: bool) -> Vec<u8> {
    let mut bytes = modern_fixture::database(code);
    let mut definition = base::definition(
        &[("Id", 4, 0, 4), ("Score", 0x12, 4, 4), ("Cached", 4, 0, 39)],
        4,
        11 << 8,
    );
    support::put32(&mut definition, 63 + 25 + 11, 7);
    definition[63 + 25 + 15] = 7;
    definition[63 + 50 + 15] = 2;
    definition[63 + 50 + 16] = 0xc0;
    support::put16(&mut definition, 63 + 50 + 7, 0);
    support::put16(&mut definition, 43, 1);
    bytes[10 * PAGE..11 * PAGE].copy_from_slice(&definition);
    let rows = IDS
        .iter()
        .zip(CACHES)
        .enumerate()
        .map(|(slot, (id, cached))| {
            let key = [Some(10_i32), None, Some(20), Some(30)][slot];
            let mut body = 3_u16.to_le_bytes().to_vec();
            body.extend(id.to_le_bytes());
            body.extend(key.unwrap_or(777).to_le_bytes());
            if let Some(value) = cached {
                body.extend(cache(value));
            }
            body.extend((body.len() as u16).to_le_bytes());
            body.extend(10_u16.to_le_bytes());
            body.extend(1_u16.to_le_bytes());
            body.push(1 | (u8::from(key.is_some()) << 1) | (u8::from(cached.is_some()) << 2));
            assert_eq!(body.len(), if cached.is_some() { 44 } else { 17 });
            (0, body)
        })
        .collect::<Vec<_>>();
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&support::data_page(10, &rows));
    install_cached_properties(&mut bytes);
    install_index(&mut bytes, descending);
    if forwarded {
        forward(&mut bytes);
    }
    bytes
}

fn name(value: &str) -> Vec<u8> {
    let text = value
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    [(text.len() as u16).to_le_bytes().as_slice(), &text].concat()
}

fn block(kind: u16, body: &[u8]) -> Vec<u8> {
    [
        ((body.len() + 6) as u32).to_le_bytes().as_slice(),
        &kind.to_le_bytes(),
        body,
    ]
    .concat()
}

fn install_cached_properties(bytes: &mut [u8]) {
    let expression = "1/0"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let property = [
        ((expression.len() + 8) as u16).to_le_bytes().as_slice(),
        &[0, 10, 0, 0],
        &(expression.len() as u16).to_le_bytes(),
        &expression,
    ]
    .concat();
    let label = name("Cached");
    let group = [
        ((label.len() + 4) as u32).to_le_bytes().as_slice(),
        &label,
        &property,
        &[9, 0, 0, 2, 1, 0, 1, 0, 4],
    ]
    .concat();
    let properties = [
        b"MR2\0".as_slice(),
        &block(0x80, &[name("Expression"), name("ResultType")].concat()),
        &block(1, &group),
    ]
    .concat();
    // Preserve all six catalog rows and their complex mappings. Extend only the
    // Items row with an actual inline LvProp and its native variable footer.
    let mut rows = Vec::new();
    for slot in 0..6 {
        let start = row_start(bytes, 4, slot);
        let end = if slot == 0 {
            5 * PAGE
        } else {
            row_start(bytes, 4, slot - 1)
        };
        let mut body = bytes[start..end].to_vec();
        if slot == 2 {
            body.truncate(body.len() - 7);
            support::put16(&mut body, 0, 6);
            let property_start = body.len() as u16;
            body.extend((0x80000000 | properties.len() as u32).to_le_bytes());
            body.extend([0; 8]);
            body.extend(&properties);
            body.extend((body.len() as u16).to_le_bytes());
            body.extend(property_start.to_le_bytes());
            body.extend(16_u16.to_le_bytes());
            body.extend(2_u16.to_le_bytes());
            body.push(0x3f);
        }
        rows.push((0, body));
    }
    bytes[4 * PAGE..5 * PAGE].copy_from_slice(&support::data_page(2, &rows));
}

fn install_index(bytes: &mut Vec<u8>, descending: bool) {
    bytes.resize(26 * PAGE, 0);
    let page = &mut bytes[10 * PAGE..11 * PAGE];
    let count = usize::from(u16::from_le_bytes(page[45..47].try_into().unwrap()));
    let fields_end = 63 + count * 25;
    let mut names_end = fields_end;
    for _ in 0..count {
        let width = usize::from(u16::from_le_bytes(
            page[names_end..names_end + 2].try_into().unwrap(),
        ));
        names_end += width + 2;
    }
    page.copy_within(63..names_end, 75);
    page[63..75].fill(0);
    support::put32(page, 16, 4);
    support::put32(page, 47, 1);
    support::put32(page, 51, 1);
    support::put32(page, 67, 3);
    let physical = names_end + 12;
    page[physical..physical + 80].fill(0);
    for slot in 0..10 {
        support::put16(page, physical + 4 + slot * 3, 0xffff);
    }
    support::put16(page, physical + 4, 0);
    page[physical + 6] = u8::from(!descending);
    support::put32(page, physical + 34, 25 << 8);
    support::put32(page, physical + 38, ROOT as u32);
    page[physical + 46] = 0x80;
    let label = name("IdOrder");
    page[physical + 80..physical + 80 + label.len()].copy_from_slice(&label);
    let mut entries = IDS
        .iter()
        .enumerate()
        .map(|(slot, id)| {
            let mut wire = vec![if descending { 128 } else { 127 }];
            wire.extend(
                ((*id as u32) ^ 0x80000000)
                    .to_be_bytes()
                    .map(|byte| if descending { !byte } else { byte }),
            );
            wire.extend((12_u32 << 8 | slot as u32).to_be_bytes());
            wire
        })
        .collect::<Vec<_>>();
    entries.sort();
    let mut leaf = vec![0; PAGE];
    leaf[..2].copy_from_slice(&[4, 1]);
    support::put16(&mut leaf, 2, 3580);
    support::put32(&mut leaf, 4, 10);
    for (position, wire) in entries.iter().enumerate() {
        leaf[480 + position * 9..489 + position * 9].copy_from_slice(wire);
        let end = (position + 1) * 9;
        leaf[27 + end / 8] |= 1 << (end % 8);
    }
    bytes[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&leaf);
    let mut map = vec![0; 69];
    map[8] = 1;
    bytes[25 * PAGE..26 * PAGE].copy_from_slice(&support::data_page(1, &[(0, map)]));
}

fn forward(bytes: &mut Vec<u8>) {
    let start = row_start(bytes, 12, 2);
    let end = row_start(bytes, 12, 1);
    let original = bytes[start..end].to_vec();
    let mut rows = Vec::new();
    for slot in 0..4 {
        let at = row_start(bytes, 12, slot);
        let end = if slot == 0 {
            13 * PAGE
        } else {
            row_start(bytes, 12, slot - 1)
        };
        if slot == 2 {
            let mut stub = vec![0; end - at];
            stub[..4].copy_from_slice(&(HIDDEN as u32 * 256).to_le_bytes());
            rows.push((0x4000, stub));
        } else {
            rows.push((0, bytes[at..end].to_vec()));
        }
    }
    bytes.resize(27 * PAGE, 0);
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&support::data_page(10, &rows));
    bytes[HIDDEN * PAGE..(HIDDEN + 1) * PAGE]
        .copy_from_slice(&support::data_page(10, &[(0x8000, original)]));
    let mut map = vec![0; 69];
    for page in [12, HIDDEN] {
        map[5 + page / 8] |= 1 << (page % 8);
    }
    bytes[11 * PAGE..12 * PAGE].copy_from_slice(&support::data_page(1, &[(0, map)]));
}
