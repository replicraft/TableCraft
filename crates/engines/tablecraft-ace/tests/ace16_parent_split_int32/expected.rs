//! Independent literal whole-image expectation; no production page encoders.
use super::*;

pub fn parent_source(code: u8) -> Model {
    let mut scores = (1..=600)
        .map(|offset| Some(i32::MIN + offset))
        .collect::<Vec<_>>();
    scores.extend((0..301).map(|position| {
        Some(if position == 0 {
            -1
        } else {
            (position as i64 * i32::MAX as i64 / 301) as i32
        })
    }));
    let mut counts = vec![600];
    counts.extend(vec![1; 301]);
    source(code, scores, counts, vec![false], false)
}

fn bound(value: &[u8], number: usize) -> Vec<u8> {
    let mut bytes = value.to_vec();
    bytes.extend((number as u32).to_be_bytes());
    bytes
}

pub fn parent_expected(model: &Model) -> Vec<u8> {
    let mut after = model.bytes.clone();
    after.extend(vec![0; 3 * PAGE]);
    let mut scores = model.scores.clone();
    scores[0] = Some(i32::MAX);
    let keys = ordered(&scores, false);
    let at = field_offset(&model.bytes, 0);
    after[at..at + 4].copy_from_slice(&i32::MAX.to_le_bytes());
    let mut leaves = Vec::new();
    let mut offset = 0;
    for (position, count) in model.counts.iter().copied().enumerate() {
        let entries = &keys[offset..offset + count];
        offset += count;
        if position == 0 {
            leaves.push((model.eof, entries[..300].to_vec(), true));
            leaves.push((model.leaves[0][position], entries[300..].to_vec(), false));
        } else {
            leaves.push((model.leaves[0][position], entries.to_vec(), false));
        }
    }
    for (position, (number, entries, fresh)) in leaves.iter().enumerate() {
        let mut seed = if *fresh {
            let mut bytes = vec![0; PAGE];
            bytes[..2].copy_from_slice(&[4, 1]);
            support::put32(&mut bytes, 4, 10);
            bytes
        } else {
            model.bytes[number * PAGE..(number + 1) * PAGE].to_vec()
        };
        let prefix = prefix_for(root_model::get16(&seed, PREFIX) as usize, entries).unwrap();
        support::put32(
            &mut seed,
            PREVIOUS,
            position
                .checked_sub(1)
                .map_or(0, |prior| leaves[prior].0 as u32),
        );
        support::put32(
            &mut seed,
            NEXT,
            leaves.get(position + 1).map_or(0, |next| next.0 as u32),
        );
        after[number * PAGE..(number + 1) * PAGE].copy_from_slice(&repack(&seed, entries, prefix));
    }
    // 303 children split by child count:151 left,152 right. Each lower node
    // uses a tail pointer, so150 and151 bounded entries are stored respectively.
    let middle = leaves.len() / 2;
    assert_eq!((middle, leaves.len() - middle), (151, 152));
    let left_number = model.eof + 2;
    let right_number = model.eof + 1;
    let mut maxima = Vec::new();
    for (left, number, children, previous, next) in [
        (true, left_number, &leaves[..middle], 0, right_number),
        (false, right_number, &leaves[middle..], left_number, 0),
    ] {
        let entries = children[..children.len() - 1]
            .iter()
            .map(|(number, values, _)| bound(values.last().unwrap(), *number))
            .collect::<Vec<_>>();
        let mut seed = vec![0; PAGE];
        seed[..2].copy_from_slice(&[3, 1]);
        support::put32(&mut seed, 4, 10);
        seed[HEIGHT] = 1;
        support::put32(&mut seed, PREVIOUS, previous as u32);
        support::put32(&mut seed, NEXT, next as u32);
        support::put32(&mut seed, TAIL, children.last().unwrap().0 as u32);
        after[number * PAGE..(number + 1) * PAGE].copy_from_slice(&repack(&seed, &entries, 0));
        let maximum = bound(children.last().unwrap().1.last().unwrap(), number);
        maxima.push(maximum);
        assert_eq!(entries.len(), if left { 150 } else { 151 });
    }
    let number = ROOTS[0];
    let mut root = model.bytes[number * PAGE..(number + 1) * PAGE].to_vec();
    root[HEIGHT] = 2;
    support::put32(&mut root, TAIL, 0);
    after[number * PAGE..(number + 1) * PAGE].copy_from_slice(&repack(&root, &maxima, 0));
    for number in model.eof..model.eof + 3 {
        for (map_page, set) in [(MAPS[0], true), (1, false)] {
            let at = root_model::map_byte(&model.bytes, map_page, number);
            let mask = 1 << (number % 8);
            if set {
                after[at] |= mask;
            } else {
                after[at] &= !mask;
            }
        }
    }
    after
}
