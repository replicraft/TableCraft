use super::*;

fn wire(kinds: &[u8], order: &[(usize, bool)], row: &[Option<i64>], ordinal: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for &(column, descending) in order {
        let key = row[column];
        bytes.push(match (key, descending) {
            (None, false) => 0,
            (None, true) => 255,
            (Some(_), false) => 127,
            (Some(_), true) => 128,
        });
        if let Some(key) = key {
            let key = match kinds[column] {
                2 => vec![key as u8],
                3 => ((key as u16) ^ 0x8000).to_be_bytes().to_vec(),
                4 => ((key as u32) ^ 0x80000000).to_be_bytes().to_vec(),
                _ => ((key as u64) ^ 0x8000000000000000).to_be_bytes().to_vec(),
            };
            bytes.extend(
                key.into_iter()
                    .map(|byte| if descending { !byte } else { byte }),
            );
        }
    }
    bytes.extend(((12_u32 << 8) | ordinal as u32).to_be_bytes());
    bytes
}
fn page(height: u8, entries: Vec<Vec<u8>>, previous: u32, next: u32, tail: u32) -> Vec<u8> {
    let mut bytes = vec![0xa7; PAGE_BYTES];
    bytes[..480].fill(0);
    bytes[0] = if height == 0 { 4 } else { 3 };
    bytes[1] = 1;
    bytes[26] = height;
    support::put32(&mut bytes, 4, 10);
    support::put32(&mut bytes, 12, previous);
    support::put32(&mut bytes, 16, next);
    support::put32(&mut bytes, 20, tail);
    let mut end = 0;
    for entry in entries {
        bytes[480 + end..480 + end + entry.len()].copy_from_slice(&entry);
        end += entry.len();
        bytes[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(&mut bytes, 2, (PAGE_BYTES - 480 - end) as u16);
    bytes
}
fn database(
    code: u8,
    kinds: &[u8],
    order: &[(usize, bool)],
    rows: &[Vec<Option<i64>>],
    bounded: bool,
) -> Vec<u8> {
    let mut bytes = model::with_alias(
        numeric_fixture::database(code, kinds, order, rows, 0),
        kinds.len(),
    );
    bytes.resize(20 * PAGE_BYTES, 0);
    bytes[13 * PAGE_BYTES..14 * PAGE_BYTES].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0xc0, 0x0f])],
    ));
    let mut entries = rows
        .iter()
        .enumerate()
        .map(|(i, row)| wire(kinds, order, row, i))
        .collect::<Vec<_>>();
    entries.sort();
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
    for (number, data) in if bounded {
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
    } {
        bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES].copy_from_slice(&data);
    }
    for (i, (number, previous, next)) in [(15, 0, 16), (16, 15, 19), (19, 16, 0)]
        .into_iter()
        .enumerate()
    {
        bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES].copy_from_slice(&page(
            0,
            vec![entries[i].clone()],
            previous,
            next,
            0,
        ));
    }
    bytes
}

#[test]
fn partial_null_tuples_repack_across_deep_bounded_and_tail_subtrees() {
    let kinds = [4, 3, 0x13, 5];
    let rows = vec![
        vec![None, Some(20), Some(i64::MIN), Some(1)],
        vec![Some(3), None, Some(i64::MAX), None],
        vec![Some(3), Some(1), None, Some(-1)],
    ];
    for code in [5, 6] {
        for reverse in [false, true] {
            for bounded in [false, true] {
                let order = [(1, reverse), (0, !reverse), (3, reverse), (2, !reverse)];
                let bytes = database(code, &kinds, &order, &rows, bounded);
                for (column, row, new) in [
                    (0, 1, None),
                    (1, 1, Some(7)),
                    (2, 0, Some(i64::MAX)),
                    (3, 2, None),
                ] {
                    let candidate =
                        preview(&bytes, &kinds, column, row, rows[row][column], new).unwrap();
                    let mut expected = rows.clone();
                    expected[row][column] = new;
                    verify(
                        &bytes,
                        candidate.candidate_bytes(),
                        &kinds,
                        &order,
                        &expected,
                        column,
                        &[row],
                    );
                }
            }
        }
    }
}
