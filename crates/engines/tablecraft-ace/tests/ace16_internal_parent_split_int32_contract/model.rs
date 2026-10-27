use super::*;

pub const PAGE: usize = 4096;
pub const ROOT: usize = 13;
pub const OWNED_MAP: usize = 14;
pub const RIGHT_PARENT: usize = 19;
pub const PEER_PARENT: usize = 20;
pub const PEER_LEAF: usize = 24;
pub const ENTRY: usize = 480;
pub const PREVIOUS: usize = 12;
pub const NEXT: usize = 16;
pub const TAIL: usize = 20;
pub const PREFIX: usize = 24;
pub const HEIGHT: usize = 26;

pub struct Target {
    pub base: old_model::Model,
    pub descending: bool,
    pub bounded: bool,
    pub original_leaf_counts: Vec<usize>,
    pub replacement: i32,
}

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn install(bytes: &mut [u8], number: usize, page: &[u8]) {
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

fn bound(key: &[u8], child: usize) -> Vec<u8> {
    let mut result = key.to_vec();
    result.extend((child as u32).to_be_bytes());
    result
}

fn blank(height: u8) -> Vec<u8> {
    let mut bytes = vec![0; PAGE];
    bytes[..2].copy_from_slice(&[if height == 0 { 4 } else { 3 }, 1]);
    support::put32(&mut bytes, 4, 10);
    bytes[HEIGHT] = height;
    bytes
}

pub fn source(code: u8, descending: bool, bounded: bool) -> Target {
    build_source(code, descending, bounded, false)
}

pub fn tied_source(descending: bool) -> Target {
    build_source(6, descending, false, true)
}

fn build_source(code: u8, descending: bool, bounded: bool, ties: bool) -> Target {
    let peers = if bounded { 300 } else { 301 };
    let mut scores = (1..=600)
        .map(|offset| Some(i32::MIN + if ties { (offset - 1) / 2 } else { offset }))
        .collect::<Vec<_>>();
    scores.extend((0..peers).map(|position| {
        Some(if position == 0 {
            -1
        } else {
            (position as i64 * i32::MAX as i64 / 301) as i32
        })
    }));
    if descending {
        for score in &mut scores {
            *score = score.map(|value| -1 - value);
        }
    }
    let mut counts = vec![600];
    counts.extend(vec![1; peers]);
    let mut base = old_model::source(code, scores, counts.clone(), vec![descending], false);
    let original_root = base.bytes[ROOT * PAGE..(ROOT + 1) * PAGE].to_vec();
    let mut parent = original_root.clone();
    let mut entries = old_model::decoded(&base.bytes, ROOT);
    if bounded {
        let last = *base.leaves[0].last().unwrap();
        let keys = old_model::decoded(&base.bytes, last);
        entries.push(bound(keys.last().unwrap(), last));
        support::put32(&mut parent, TAIL, 0);
        parent = old_model::repack(&parent, &entries, 1);
    }
    support::put32(
        &mut parent,
        PREVIOUS,
        if descending { 0 } else { PEER_PARENT as u32 },
    );
    support::put32(
        &mut parent,
        NEXT,
        if descending { PEER_PARENT as u32 } else { 0 },
    );
    install(&mut base.bytes, RIGHT_PARENT, &parent);

    let null_row = base.scores.len();
    base.scores.push(None);
    support::put32(&mut base.bytes, 10 * PAGE + 16, base.scores.len() as u32);
    support::put32(&mut base.bytes, 10 * PAGE + 67, base.scores.len() as u32);
    for (group, values) in base.scores.chunks(255).enumerate() {
        let rows = values
            .iter()
            .enumerate()
            .map(|(slot, value)| {
                let row = group * 255 + slot;
                let mut record = vec![2, 0];
                record.extend((row as i32 + 1).to_le_bytes());
                record.extend(value.unwrap_or(0).to_le_bytes());
                record.push(if value.is_some() { 3 } else { 1 });
                (0, record)
            })
            .collect::<Vec<_>>();
        let mut page = support::data_page(10, &rows);
        support::put16(&mut page, 2, (PAGE - 14 - 13 * rows.len()) as u16);
        install(&mut base.bytes, old_model::DATA[group], &page);
    }
    let null_key = old_model::wire(null_row, None, descending);
    let first = base.leaves[0][0];
    let last = *base.leaves[0].last().unwrap();
    let mut peer_leaf = old_model::repack(&blank(0), std::slice::from_ref(&null_key), 0);
    support::put32(
        &mut peer_leaf,
        PREVIOUS,
        if descending { last as u32 } else { 0 },
    );
    support::put32(
        &mut peer_leaf,
        NEXT,
        if descending { 0 } else { first as u32 },
    );
    install(&mut base.bytes, PEER_LEAF, &peer_leaf);
    support::put32(
        &mut base.bytes,
        (if descending { last } else { first }) * PAGE + if descending { NEXT } else { PREVIOUS },
        PEER_LEAF as u32,
    );
    let mut peer = old_model::repack(&blank(1), &[bound(&null_key, PEER_LEAF)], 0);
    support::put32(
        &mut peer,
        PREVIOUS,
        if descending { RIGHT_PARENT as u32 } else { 0 },
    );
    support::put32(
        &mut peer,
        NEXT,
        if descending { 0 } else { RIGHT_PARENT as u32 },
    );
    install(&mut base.bytes, PEER_PARENT, &peer);
    let target_last = old_model::decoded(&base.bytes, last)
        .last()
        .unwrap()
        .clone();
    let mut bounds = vec![
        bound(&null_key, PEER_PARENT),
        bound(&target_last, RIGHT_PARENT),
    ];
    if descending {
        bounds.reverse();
    }
    let mut root = old_model::repack(&blank(2), &bounds, 0);
    let root_end = PAGE - root_model::get16(&root, 2) as usize;
    root[root_end..].fill(0xb6);
    install(&mut base.bytes, ROOT, &root);
    let mut owned = vec![ROOT, RIGHT_PARENT, PEER_PARENT, PEER_LEAF];
    owned.extend_from_slice(&base.leaves[0]);
    install(
        &mut base.bytes,
        OWNED_MAP,
        &support::data_page(0, &[(0, old_model::map(&owned))]),
    );
    Target {
        base,
        descending,
        bounded,
        original_leaf_counts: counts,
        replacement: if descending { i32::MIN } else { i32::MAX },
    }
}

pub fn check(target: &Target) {
    old_model::check(&target.base);
    let report = check_table(
        &mut Cursor::new(&target.base.bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.indexes().len(), 2);
}

pub fn request(target: &Target) -> Int32Update {
    Int32Update::new(0, target.base.scores[0].unwrap(), target.replacement)
}

pub fn run(target: &Target) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    root_model::capture(&target.base.bytes).preview_ace16_split_int32_updates(
        "Items",
        "Score",
        &[request(target)],
        &mut budget(),
        &|| false,
    )
}

pub fn expected(target: &Target) -> Vec<u8> {
    let source = &target.base.bytes;
    let mut after = source.clone();
    let field = old_model::field_offset(source, 0);
    after[field..field + 4].copy_from_slice(&target.replacement.to_le_bytes());
    let mut scores = target.base.scores.clone();
    scores[0] = Some(target.replacement);
    let mut keys = old_model::ordered(&scores, target.descending);
    let null_key = if target.descending {
        keys.pop().unwrap()
    } else {
        keys.remove(0)
    };
    assert_eq!(null_key.len(), 5);
    let new_leaf = target.base.eof;
    let new_parent = new_leaf + 1;
    after.resize((new_parent + 1) * PAGE, 0);
    let mut children = Vec::new();
    let mut offset = 0;
    for (position, &count) in target.original_leaf_counts.iter().enumerate() {
        let number = target.base.leaves[0][position];
        let entries = keys[offset..offset + count].to_vec();
        offset += count;
        if position == 0 {
            children.push((new_leaf, entries[..300].to_vec(), true));
            children.push((number, entries[300..].to_vec(), false));
        } else {
            children.push((number, entries, false));
        }
    }
    for (position, (number, entries, fresh)) in children.iter().enumerate() {
        let mut page = if *fresh {
            blank(0)
        } else {
            source[number * PAGE..(number + 1) * PAGE].to_vec()
        };
        let old_prefix = root_model::get16(&page, PREFIX) as usize;
        let prefix = old_model::prefix_for(old_prefix, entries).unwrap();
        support::put32(
            &mut page,
            PREVIOUS,
            if position == 0 {
                if target.descending {
                    0
                } else {
                    PEER_LEAF as u32
                }
            } else {
                children[position - 1].0 as u32
            },
        );
        support::put32(
            &mut page,
            NEXT,
            children
                .get(position + 1)
                .map(|child| child.0 as u32)
                .unwrap_or(if target.descending {
                    PEER_LEAF as u32
                } else {
                    0
                }),
        );
        install(
            &mut after,
            *number,
            &old_model::repack(&page, entries, prefix),
        );
    }
    if !target.descending {
        support::put32(&mut after, PEER_LEAF * PAGE + NEXT, new_leaf as u32);
    }
    let middle = children.len() / 2;
    let left = &children[..middle];
    let right = &children[middle..];
    for (number, group, fresh, bounded) in [
        (new_parent, left, true, false),
        (RIGHT_PARENT, right, false, target.bounded),
    ] {
        let mut bounds = Vec::new();
        for (position, (child, entries, _)) in group.iter().enumerate() {
            if bounded || position + 1 < group.len() {
                bounds.push(bound(entries.last().unwrap(), *child));
            }
        }
        let mut page = if fresh {
            blank(1)
        } else {
            source[number * PAGE..(number + 1) * PAGE].to_vec()
        };
        let prefix =
            old_model::prefix_for(root_model::get16(&page, PREFIX) as usize, &bounds).unwrap();
        support::put32(
            &mut page,
            PREVIOUS,
            if fresh {
                if target.descending {
                    0
                } else {
                    PEER_PARENT as u32
                }
            } else {
                new_parent as u32
            },
        );
        support::put32(
            &mut page,
            NEXT,
            if fresh {
                RIGHT_PARENT as u32
            } else if target.descending {
                PEER_PARENT as u32
            } else {
                0
            },
        );
        support::put32(
            &mut page,
            TAIL,
            if bounded {
                0
            } else {
                group.last().unwrap().0 as u32
            },
        );
        install(
            &mut after,
            number,
            &old_model::repack(&page, &bounds, prefix),
        );
    }
    if !target.descending {
        support::put32(&mut after, PEER_PARENT * PAGE + NEXT, new_parent as u32);
    }
    let mut root_bounds = vec![
        bound(&null_key, PEER_PARENT),
        bound(left.last().unwrap().1.last().unwrap(), new_parent),
        bound(right.last().unwrap().1.last().unwrap(), RIGHT_PARENT),
    ];
    if target.descending {
        let peer = root_bounds.remove(0);
        root_bounds.push(peer);
    }
    install(
        &mut after,
        ROOT,
        &old_model::repack(&source[ROOT * PAGE..(ROOT + 1) * PAGE], &root_bounds, 0),
    );
    for number in [new_leaf, new_parent] {
        for (map_page, set) in [(OWNED_MAP, true), (1, false)] {
            let at = root_model::map_byte(source, map_page, number);
            let mask = 1 << (number % 8);
            assert_eq!(source[at] & mask != 0, !set);
            if set {
                after[at] |= mask;
            } else {
                after[at] &= !mask;
            }
        }
    }
    after
}
