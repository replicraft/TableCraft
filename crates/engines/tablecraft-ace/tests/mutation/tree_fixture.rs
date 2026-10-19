use super::{index_fixture, support};

pub fn database(descending: bool, nullable: bool, deep: bool) -> Vec<u8> {
    let mut bytes = index_fixture::database(descending, false, nullable);
    bytes.resize(if deep { 20 } else { 17 } * 4096, 0);
    bytes[14 * 4096..15 * 4096].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0xa0, if deep { 0x0f } else { 1 }])],
    ));
    let mut keys = [
        (if nullable { None } else { Some(10) }, 0),
        (Some(20), 1),
        (Some(30), 2),
    ];
    keys.sort_by(|a, b| {
        let order = a.0.cmp(&b.0);
        if descending { order.reverse() } else { order }
    });
    let entries = keys.map(|(key, slot)| entry(key, slot, descending));
    if deep {
        let mut separator = entries[1].clone();
        separator.extend(17_u32.to_be_bytes());
        bytes[13 * 4096..14 * 4096].copy_from_slice(&page(2, vec![separator], 0, 0, 18));
        let mut separator = entries[0].clone();
        separator.extend(15_u32.to_be_bytes());
        bytes[17 * 4096..18 * 4096].copy_from_slice(&page(1, vec![separator], 0, 0, 16));
        // A nonempty tail subtree with zero explicit separators is intentional.
        bytes[18 * 4096..19 * 4096].copy_from_slice(&page(1, vec![], 0, 0, 19));
        bytes[15 * 4096..16 * 4096].copy_from_slice(&page(0, vec![entries[0].clone()], 0, 16, 0));
        bytes[16 * 4096..17 * 4096].copy_from_slice(&page(0, vec![entries[1].clone()], 15, 19, 0));
        bytes[19 * 4096..20 * 4096].copy_from_slice(&page(0, vec![entries[2].clone()], 16, 0, 0));
    } else {
        let mut separator = entries[1].clone();
        separator.extend(15_u32.to_be_bytes());
        bytes[13 * 4096..14 * 4096].copy_from_slice(&page(1, vec![separator], 0, 0, 16));
        bytes[15 * 4096..16 * 4096].copy_from_slice(&page(0, entries[..2].to_vec(), 0, 16, 0));
        bytes[16 * 4096..17 * 4096].copy_from_slice(&page(0, vec![entries[2].clone()], 15, 0, 0));
    }
    bytes
}

// Author a second valid node topology: every child has an explicit upper
// bound, so there is no unbounded tail pointer. Leaves and ownership are intact.
pub fn bounded_database(descending: bool, nullable: bool, deep: bool) -> Vec<u8> {
    let mut bytes = database(descending, nullable, deep);
    let mut keys = [
        (if nullable { None } else { Some(10) }, 0),
        (Some(20), 1),
        (Some(30), 2),
    ];
    keys.sort_by(|a, b| {
        let order = a.0.cmp(&b.0);
        if descending { order.reverse() } else { order }
    });
    let entries = keys.map(|(key, slot)| entry(key, slot, descending));
    let node = |height, children: &[(usize, u32)]| {
        let entries = children
            .iter()
            .map(|(key, child)| {
                let mut entry = entries[*key].clone();
                entry.extend(child.to_be_bytes());
                entry
            })
            .collect();
        page(height, entries, 0, 0, 0)
    };
    if deep {
        bytes[13 * 4096..14 * 4096].copy_from_slice(&node(2, &[(1, 17), (2, 18)]));
        bytes[17 * 4096..18 * 4096].copy_from_slice(&node(1, &[(0, 15), (1, 16)]));
        bytes[18 * 4096..19 * 4096].copy_from_slice(&node(1, &[(2, 19)]));
    } else {
        bytes[13 * 4096..14 * 4096].copy_from_slice(&node(1, &[(1, 15), (2, 16)]));
    }
    bytes
}

fn entry(key: Option<i32>, slot: u32, descending: bool) -> Vec<u8> {
    let mut bytes = vec![match (key, descending) {
        (None, false) => 0,
        (None, true) => 0xff,
        (Some(_), false) => 0x7f,
        (Some(_), true) => 0x80,
    }];
    if let Some(key) = key {
        bytes.extend(
            ((key as u32) ^ 0x80000000)
                .to_be_bytes()
                .map(|byte| if descending { !byte } else { byte }),
        );
    }
    bytes.extend(((12_u32 << 8) | slot).to_be_bytes());
    bytes
}

fn page(height: u8, entries: Vec<Vec<u8>>, previous: u32, next: u32, tail: u32) -> Vec<u8> {
    let mut bytes = vec![0xa7; 4096];
    bytes[..480].fill(0);
    bytes[..2].copy_from_slice(&[if height == 0 { 4 } else { 3 }, 1]);
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
    support::put16(&mut bytes, 2, (4096 - 480 - end) as u16);
    bytes
}
