use crate::{
    base, complex_fixture,
    support::{data_page, put16},
};

pub(super) const PAGE_BYTES: usize = 4096;
pub(super) const TEMPLATE_PAGE: usize = 17;
pub(super) const FLAT_PAGE: usize = 20;
pub(super) const CHILD_PAGE: usize = 22;
pub(super) const FIELD_START: usize = 63;
pub(super) const FIELD_BYTES: usize = 25;
// Independent Python-zlib golden retained by the original wrapper tests.
pub(super) const PAYLOAD: &[u8] = &[
    1, 0, 0, 0, 24, 0, 0, 0, 120, 94, 19, 97, 96, 96, 96, 4, 98, 22, 32, 78, 98, 200, 100, 200, 3,
    210, 12, 140, 76, 255, 1, 15, 116, 2, 85,
];

pub(super) fn definition(
    fields: &[(&str, u8, u16, u16)],
    records: u32,
    usage: u32,
    conceptual: bool,
) -> Vec<u8> {
    let mut page = base::definition(fields, records, usage);
    let mut variable = 0;
    for (i, (_, kind, _, _)) in fields.iter().enumerate() {
        let at = FIELD_START + i * FIELD_BYTES;
        if matches!(kind, 10..=12) {
            page[at + 15] = 2;
            put16(&mut page, at + 7, variable);
            variable += 1;
        }
        if conceptual {
            page[at + 16] = 0x10;
        }
    }
    put16(&mut page, 43, variable);
    page
}
pub(super) fn template() -> Vec<(&'static str, u8, u16, u16)> {
    vec![
        ("FileURL", 12, 0, 0),
        ("FileName", 10, 0, 510),
        ("FileType", 10, 0, 510),
        ("FileData", 11, 0, 0),
        ("FileTimeStamp", 8, 0, 8),
        ("FileFlags", 4, 8, 4),
    ]
}
fn inline(data: &[u8]) -> Vec<u8> {
    let mut value = (0x8000_0000 | data.len() as u32).to_le_bytes().to_vec();
    value.extend([0; 8]);
    value.extend(data);
    value
}
fn text(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
pub(super) fn database() -> Vec<u8> {
    with_payloads(PAYLOAD, &[])
}
pub(super) fn with_payloads(first: &[u8], last: &[u8]) -> Vec<u8> {
    let mut bytes = complex_fixture::with_mappings(vec![complex_fixture::mapping(7, 10, 20, 17)]);
    bytes[10 * PAGE_BYTES + FIELD_START + FIELD_BYTES + 15] = 7;
    bytes[TEMPLATE_PAGE * PAGE_BYTES..(TEMPLATE_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&definition(&template(), 0, 18 << 8, true));
    let fields = [
        ("Owner", 4, 0, 4),
        ("ChildID", 4, 4, 4),
        ("FileURL", 12, 0, 0),
        ("FileName", 10, 0, 510),
        ("FileType", 10, 0, 510),
        ("FileData", 11, 0, 0),
        ("FileTimeStamp", 8, 8, 8),
        ("FileFlags", 4, 16, 4),
    ];
    let mut flat = definition(&fields, 3, 21 << 8, false);
    flat[FIELD_START + 16] = 8;
    flat[FIELD_START + FIELD_BYTES + 15] = 7;
    bytes[FLAT_PAGE * PAGE_BYTES..(FLAT_PAGE + 1) * PAGE_BYTES].copy_from_slice(&flat);
    let rows = (0..3)
        .map(|i| {
            let mut row = 8_u16.to_le_bytes().to_vec();
            row.extend((if i == 2 { 20_i32 } else { 10 }).to_le_bytes());
            row.extend((i + 1_i32).to_le_bytes());
            row.extend((-0.25_f64).to_bits().to_le_bytes());
            row.extend(i32::MIN.to_le_bytes());
            let fields = match i {
                0 => [
                    inline(&text("../unsafe?query")),
                    text("../é🦀\0.bin"),
                    text("field/type"),
                    inline(first),
                ],
                1 => std::array::from_fn(|_| Vec::new()),
                _ => [inline(&[]), Vec::new(), Vec::new(), inline(last)],
            };
            let mut offsets = vec![row.len() as u16];
            for field in fields {
                row.extend(field);
                offsets.push(row.len() as u16);
            }
            for offset in offsets.into_iter().rev() {
                row.extend(offset.to_le_bytes());
            }
            row.extend(4_u16.to_le_bytes());
            row.push(if i == 1 { 3 } else { 0xff });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[CHILD_PAGE * PAGE_BYTES..(CHILD_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(FLAT_PAGE as u32, &rows));
    bytes
}
