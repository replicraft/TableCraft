//! Independent catalog property carrier access and fitted append at actual roots.
use super::{packed_record::append_record, raw_forwarding_chain};

const PAGE: usize = 4096;

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

pub(in crate::fixtures::access) fn body(before: &[u8], root: usize) -> Vec<u8> {
    let row = raw_forwarding_chain::record(before, 4, root);
    let footer = row.len() - 5;
    let start = word(row, footer - 18);
    let end = word(row, footer - 20);
    assert!(end - start >= 12);
    let tagged = u32::from_le_bytes(row[start..start + 4].try_into().unwrap());
    let length = (tagged & 0x3fff_ffff) as usize;
    if tagged & 0xc000_0000 == 0x8000_0000 {
        assert_eq!(end - start, 12 + length);
        row[start + 12..end].to_vec()
    } else {
        assert_eq!(tagged & 0xc000_0000, 0x4000_0000);
        assert_eq!(end - start, 12);
        let reference = u32::from_le_bytes(row[start + 4..start + 8].try_into().unwrap());
        let property = (reference >> 8) as usize;
        let slot = (reference & 255) as usize;
        let begin = word(before, property * PAGE + 14 + 2 * slot);
        let finish = if slot == 0 {
            PAGE
        } else {
            word(before, property * PAGE + 12 + 2 * slot)
        };
        assert_eq!(finish - begin, length);
        before[property * PAGE + begin..property * PAGE + finish].to_vec()
    }
}

pub(in crate::fixtures::access) fn append(
    before: &[u8],
    mut after: Vec<u8>,
    root: usize,
    body: &[u8],
    property_page: u32,
) -> Vec<u8> {
    let range = raw_forwarding_chain::terminal_range(before, 4, root);
    let row = &before[range.clone()];
    let footer = row.len() - 5;
    let start = word(row, footer - 18);
    assert!(word(row, footer - 20) - start >= 12);
    let page = property_page as usize * PAGE;
    let slot = word(before, page + 12);
    append_record(&mut after[page..page + PAGE], body);
    let descriptor = range.start + start;
    after[descriptor..descriptor + 4]
        .copy_from_slice(&(0x4000_0000 | body.len() as u32).to_le_bytes());
    after[descriptor + 4..descriptor + 8]
        .copy_from_slice(&((property_page << 8) | slot as u32).to_le_bytes());
    after[descriptor + 8..descriptor + 12].fill(0);
    // The twelve-byte descriptor consumes less than the prior inline footprint.
    // Preserve ignored original inline bytes and the complete physical row size.
    for variable in 9..=11 {
        let at = range.start + footer - 2 * (variable + 1);
        after[at..at + 2].copy_from_slice(&((start + 12) as u16).to_le_bytes());
    }
    after[range.end - 2] |= 0x40;
    after
}

/// Literal inline LvProp hint map belonging to these independently known
/// seventeen-column MSysObjects definitions; no production allocator is called.
pub(in crate::fixtures::access) fn candidate(before: &[u8], body_bytes: usize) -> u32 {
    let map = 3 * PAGE + word(before, 3 * PAGE + 14 + 2 * 17);
    assert_eq!(before[map], 0);
    let base = u32::from_le_bytes(before[map + 1..map + 5].try_into().unwrap()) as usize;
    for page in (0..(before.len() / PAGE)).rev() {
        if page < base
            || page >= base + 64
            || before[map + 5 + (page - base) / 8] & (1 << ((page - base) % 8)) == 0
        {
            continue;
        }
        let raw = &before[page * PAGE..(page + 1) * PAGE];
        assert_eq!(&raw[4..8], b"LVAL");
        if word(raw, 12) < 255 && word(raw, 2) >= body_bytes + 2 {
            return page as u32;
        }
    }
    panic!("authored retained source must have fitting owned/hinted property storage");
}
