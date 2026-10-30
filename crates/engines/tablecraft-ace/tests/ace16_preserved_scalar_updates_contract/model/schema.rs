//! Exact native declarations and three independently encoded retained owners.
use super::{PAGE, base, raw, records};

pub(super) const FIELDS: [(&str, u8, u16, u16); 16] = [
    ("ID", 4, 0, 4),
    ("Short_Text", 10, 0, 510),
    ("Long_Text", 12, 0, 0),
    ("Number", 4, 4, 4),
    ("Large_Number", 19, 8, 8),
    ("Date_Field", 8, 16, 8),
    ("Time_Field", 8, 24, 8),
    ("Date_Extended_Field", 20, 32, 42),
    ("Time_Extended_Field", 20, 74, 42),
    ("Currency_Field", 5, 116, 8),
    ("Yes_Field", 1, 0, 0),
    ("No_Field", 1, 0, 0),
    ("OLE_Object_Field", 11, 0, 0),
    ("Hyperlink_Field", 12, 0, 0),
    ("Attachment_Field", 18, 124, 4),
    ("Calculated", 4, 0, 39),
];

pub(super) fn definition() -> Vec<u8> {
    let mut page = base::definition(&FIELDS, 3, 11 << 8);
    let names_end = 63
        + FIELDS.len() * 25
        + FIELDS
            .iter()
            .map(|(name, ..)| 2 + name.encode_utf16().count() * 2)
            .sum::<usize>();
    page.copy_within(63..names_end, 99);
    page[63..99].fill(0);
    raw::put32(&mut page, 47, 3);
    raw::put32(&mut page, 51, 3);
    raw::put32(&mut page, 59, (11 << 8) | 1);
    raw::put16(&mut page, 43, 5);

    for (physical, history) in [17, 19, 23].into_iter().enumerate() {
        raw::put32(&mut page, 67 + physical * 12, history);
    }
    page[99 + 15] = 7;
    for (variable, number) in [1, 2, 12, 13, 15].into_iter().enumerate() {
        let at = 99 + number * 25;
        page[at + 15] = if number == 13 { 0x82 } else { 2 };
        raw::put16(&mut page, at + 7, variable as u16);
    }
    page[99 + 25 + 16] = 1;
    raw::put32(&mut page, 99 + 14 * 25 + 11, 7);
    page[99 + 14 * 25 + 15] = 7;
    page[99 + 15 * 25 + 16] = 0xc0;

    let physical_start = names_end + 36;
    for (physical, number) in [14_u16, 3, 0].into_iter().enumerate() {
        let at = physical_start + physical * 52;
        page[at..at + 52].fill(0);
        for component in 0..10 {
            raw::put16(&mut page, at + 4 + component * 3, 0xffff);
        }
        raw::put16(&mut page, at + 4, number);
        page[at + 6] = 1;
        raw::put32(&mut page, at + 34, (30 << 8) | physical as u32);
        raw::put32(&mut page, at + 38, 27 + physical as u32);
        page[at + 46] = if physical == 1 { 0x80 } else { 0x89 };
    }

    let logical_start = physical_start + 3 * 52;
    let mut tail = logical_start + 3 * 28;
    for (physical, name) in ["AttachmentKey", "NumberKey", "PrimaryKey"]
        .into_iter()
        .enumerate()
    {
        let at = logical_start + physical * 28;
        page[at..at + 28].fill(0);
        raw::put32(&mut page, at + 4, physical as u32);
        raw::put32(&mut page, at + 8, physical as u32);
        page[at + 23] = u8::from(physical == 2);
        let name = records::text(name);
        raw::put16(&mut page, tail, name.len() as u16);
        tail += 2;
        page[tail..tail + name.len()].copy_from_slice(&name);
        tail += name.len();
    }
    for (number, owned) in [(2, 2), (12, 4), (13, 6)] {
        raw::put16(&mut page, tail, number);
        raw::put32(&mut page, tail + 2, (11 << 8) | owned);
        raw::put32(&mut page, tail + 6, (11 << 8) | (owned + 1));
        tail += 10;
    }
    raw::put16(&mut page, tail, 0xffff);
    page
}

pub(super) fn index_page(physical: usize) -> Vec<u8> {
    let mut page = vec![0x9b; PAGE];
    page[..480].fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    raw::put32(&mut page, 4, 10);
    let mut end = 0;
    for ordinal in 0..3 {
        let value = if physical == 0 {
            (ordinal + 1) * 10
        } else {
            ordinal + 1
        };
        let mut entry = vec![0x7f];
        entry.extend(((value as u32) ^ 0x8000_0000).to_be_bytes());
        entry.extend(((12 << 8) | ordinal as u32).to_be_bytes());
        page[480 + end..480 + end + entry.len()].copy_from_slice(&entry);
        end += entry.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    raw::put16(&mut page, 2, (PAGE - 480 - end) as u16);
    page
}
