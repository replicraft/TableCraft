//! Exact independent source/fresh INDEX framing and inactive-byte retention.
use super::PAGE;
use tablecraft_testkit::access as raw;

pub(crate) fn index_page(
    before: Option<&[u8]>,
    entries: &[Vec<u8>],
    links: (u32, u32),
    tail: u32,
    branch: bool,
) -> Vec<u8> {
    let mut page = before.map_or_else(|| vec![0; PAGE], <[u8]>::to_vec);
    page[0] = if branch { 3 } else { 4 };
    page[1] = 1;
    raw::put32(&mut page, 4, 10);
    raw::put32(&mut page, 12, links.0);
    raw::put32(&mut page, 16, links.1);
    raw::put32(&mut page, 20, tail);
    raw::put16(&mut page, 24, 0);
    page[26] = u8::from(branch);
    page[27..480].fill(0);
    let mut length = 0;
    for entry in entries {
        assert!(length + entry.len() <= 3616);
        page[480 + length..480 + length + entry.len()].copy_from_slice(entry);
        length += entry.len();
        page[27 + length / 8] |= 1 << (length % 8);
    }
    raw::put16(&mut page, 2, (3616 - length) as u16);
    page
}

pub(crate) fn separator(maximum: &[u8], child: u32) -> Vec<u8> {
    [maximum, &child.to_be_bytes()].concat()
}
