//! Independent eight-byte signed key goldens, sharing only synthetic page framing.
use super::{ignored, int64_index_fixture, support};

pub const PAGE_BYTES: usize = 4096;
pub const ROOT: usize = 14;
pub const FLAGS: usize = 10 * PAGE_BYTES + 189;
const KEY_TYPE: usize = 10 * PAGE_BYTES + 100;
const USAGE: usize = 13;
const LEFT: usize = 15;
const RIGHT: usize = 16;

pub fn key(value: Option<i64>, slot: usize, descending: bool) -> Vec<u8> {
    let mut bytes = vec![match (value, descending) {
        (None, false) => 0,
        (None, true) => 255,
        (Some(_), false) => 127,
        (Some(_), true) => 128,
    }];
    if let Some(value) = value {
        bytes.extend(
            ((value as u64) ^ 0x8000_0000_0000_0000)
                .to_be_bytes()
                .map(|byte| if descending { !byte } else { byte }),
        );
    }
    bytes.extend(((12_u32 << 8) | slot as u32).to_be_bytes());
    bytes
}

pub fn database(
    code: u8,
    currency: bool,
    descending: bool,
    node: bool,
    values: &[Option<i64>],
) -> Vec<u8> {
    let mut bytes = int64_index_fixture::database(code, descending, values);
    if code == 1 {
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    }
    if currency {
        bytes[KEY_TYPE] = 0x05;
    }
    bytes[FLAGS] = 0x82;
    let mut entries: Vec<_> = values
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(slot, value)| value.map(|value| (value, slot)))
        .collect();
    entries.sort_by(|(left, a), (right, b)| {
        let order = left.cmp(right);
        (if descending { order.reverse() } else { order }).then(a.cmp(b))
    });
    let entries: Vec<_> = entries
        .into_iter()
        .map(|(value, slot)| key(Some(value), slot, descending))
        .collect();
    if node {
        assert!(entries.len() >= 2);
        bytes.resize((RIGHT + 1) * PAGE_BYTES, 0);
        let split = entries.len() / 2;
        let mut separator = entries[split - 1].clone();
        separator.extend((LEFT as u32).to_be_bytes());
        bytes[ROOT * PAGE_BYTES..(ROOT + 1) * PAGE_BYTES].copy_from_slice(&ignored::page(
            false,
            &[separator],
            0,
            0,
            RIGHT as u32,
        ));
        bytes[LEFT * PAGE_BYTES..(LEFT + 1) * PAGE_BYTES].copy_from_slice(&ignored::page(
            true,
            &entries[..split],
            0,
            RIGHT as u32,
            0,
        ));
        bytes[RIGHT * PAGE_BYTES..(RIGHT + 1) * PAGE_BYTES].copy_from_slice(&ignored::page(
            true,
            &entries[split..],
            LEFT as u32,
            0,
            0,
        ));
        bytes[USAGE * PAGE_BYTES..(USAGE + 1) * PAGE_BYTES].copy_from_slice(&support::data_page(
            1,
            &[(0, vec![0, 0, 0, 0, 0, 0, 0xc0, 1])],
        ));
    } else {
        bytes[ROOT * PAGE_BYTES..(ROOT + 1) * PAGE_BYTES]
            .copy_from_slice(&ignored::page(true, &entries, 0, 0, 0));
    }
    bytes
}
