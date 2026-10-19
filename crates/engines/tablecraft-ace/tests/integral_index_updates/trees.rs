use super::*;

// Authored uncompressed tree packets, independent of the mutation encoder.
fn entry(kind: u8, key: i64, row: u32, descending: bool) -> Vec<u8> {
    let mut bytes = vec![if descending { 0x80 } else { 0x7f }];
    let key = match kind {
        2 => vec![key as u8],
        3 => ((key as u16) ^ 0x8000).to_be_bytes().to_vec(),
        4 => ((key as u32) ^ 0x80000000).to_be_bytes().to_vec(),
        _ => ((key as u64) ^ 0x8000000000000000).to_be_bytes().to_vec(),
    };
    bytes.extend(key.into_iter().map(|b| if descending { !b } else { b }));
    bytes.extend(((12 << 8) | row).to_be_bytes());
    bytes
}
fn page(height: u8, entries: Vec<Vec<u8>>, previous: u32, next: u32, tail: u32) -> Vec<u8> {
    let mut bytes = vec![0xa7; PAGE_BYTES];
    bytes[..ENTRY_START].fill(0);
    bytes[0] = if height == 0 { 4 } else { 3 };
    bytes[1] = 1;
    bytes[26] = height;
    support::put32(&mut bytes, 4, 10);
    support::put32(&mut bytes, 12, previous);
    support::put32(&mut bytes, 16, next);
    support::put32(&mut bytes, 20, tail);
    let mut end = 0;
    for entry in entries {
        bytes[ENTRY_START + end..ENTRY_START + end + entry.len()].copy_from_slice(&entry);
        end += entry.len();
        bytes[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(&mut bytes, 2, (PAGE_BYTES - ENTRY_START - end) as u16);
    bytes
}
fn database(code: u8, kind: u8, descending: bool, bounded: bool) -> Vec<u8> {
    let mut bytes = source(code, kind, descending, &[Some(10), Some(20), Some(30)], 0);
    bytes.resize(20 * PAGE_BYTES, 0);
    bytes[13 * PAGE_BYTES..14 * PAGE_BYTES].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0xc0, 0x0f])],
    ));
    let mut keys = [(10, 0), (20, 1), (30, 2)];
    if descending {
        keys.reverse();
    }
    let entries = keys.map(|(key, row)| entry(kind, key, row, descending));
    let node = |height, children: &[(usize, u32)], tail| {
        let entries = children
            .iter()
            .map(|(key, child)| {
                let mut bytes = entries[*key].clone();
                bytes.extend(child.to_be_bytes());
                bytes
            })
            .collect();
        page(height, entries, 0, 0, tail)
    };
    let nodes = if bounded {
        [
            (14, node(2, &[(1, 17), (2, 18)], 0)),
            (17, node(1, &[(0, 15), (1, 16)], 0)),
            (18, node(1, &[(2, 19)], 0)),
        ]
    } else {
        [
            (14, node(2, &[(1, 17)], 18)),
            (17, node(1, &[(0, 15)], 16)),
            (18, node(1, &[], 19)),
        ]
    };
    for (number, page) in nodes {
        bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES].copy_from_slice(&page);
    }
    for (position, (number, previous, next)) in [(15, 0, 16), (16, 15, 19), (19, 16, 0)]
        .into_iter()
        .enumerate()
    {
        bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES].copy_from_slice(&page(
            0,
            vec![entries[position].clone()],
            previous,
            next,
            0,
        ));
    }
    bytes
}

#[test]
fn wider_keys_cross_deep_subtrees_and_update_bounded_and_tail_separators() {
    for code in [5, 6] {
        for kind in [2, 3, 0x13, 5] {
            for descending in [false, true] {
                for bounded in [false, true] {
                    let bytes = database(code, kind, descending, bounded);
                    let candidate = preview(&bytes, kind, Some(10), Some(40)).unwrap();
                    verify(
                        &bytes,
                        candidate.candidate_bytes(),
                        kind,
                        descending,
                        &[Some(40), Some(20), Some(30)],
                    );
                    let candidate2 =
                        preview(candidate.candidate_bytes(), kind, Some(40), None).unwrap();
                    verify(
                        candidate.candidate_bytes(),
                        candidate2.candidate_bytes(),
                        kind,
                        descending,
                        &[None, Some(20), Some(30)],
                    );
                }
            }
        }
    }
}
