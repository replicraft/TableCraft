use super::*;

pub const PAGE: usize = 4096;
pub const ENTRY: usize = 480;
pub const BITMAP: usize = 27;
pub const FREE: usize = 2;
pub const PREFIX: usize = 24;
pub const HEIGHT: usize = 26;
pub const PREVIOUS: usize = 12;
pub const NEXT: usize = 16;
pub const TAIL: usize = 20;
pub const DATA: [usize; 8] = [12, 15, 16, 17, 18, 21, 22, 23];
pub const ROOTS: [usize; 2] = [13, 19];
pub const MAPS: [usize; 2] = [14, 20];
pub const ROWS_PER_PAGE: usize = 255;
pub const MAP_HEADER: usize = 5;
pub const MAP_BYTES: usize = 69;
pub const SLOT_START: usize = 14;
pub const OFFSET_MASK: u16 = 0x1fff;

pub struct Model {
    pub bytes: Vec<u8>,
    pub scores: Vec<Option<i32>>,
    pub directions: Vec<bool>,
    pub leaves: Vec<Vec<usize>>,
    pub counts: Vec<usize>,
    pub bounded: bool,
    pub eof: usize,
}

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

pub fn reference(row: usize) -> u32 {
    ((DATA[row / ROWS_PER_PAGE] as u32) << 8) | (row % ROWS_PER_PAGE) as u32
}

pub fn wire(row: usize, value: Option<i32>, descending: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Some(value) = value {
        bytes.push(if descending { 128 } else { 127 });
        let key = ((value as u32) ^ 0x8000_0000).to_be_bytes();
        bytes.extend(key.map(|byte| if descending { !byte } else { byte }));
    } else {
        bytes.push(if descending { 255 } else { 0 });
    }
    bytes.extend(reference(row).to_be_bytes());
    bytes
}

pub fn ordered(scores: &[Option<i32>], descending: bool) -> Vec<Vec<u8>> {
    let mut entries = scores
        .iter()
        .copied()
        .enumerate()
        .map(|(row, value)| wire(row, value, descending))
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

pub fn common(entries: &[Vec<u8>], key_bytes: usize) -> usize {
    if entries.len() < 2 {
        return 0;
    }
    let mut result = entries[0].len().min(key_bytes);
    for entry in &entries[1..] {
        result = result.min(entry.len().min(key_bytes));
        result = entries[0][..result]
            .iter()
            .zip(&entry[..result])
            .take_while(|(left, right)| left == right)
            .count();
    }
    result
}

pub fn repack(original: &[u8], entries: &[Vec<u8>], prefix: usize) -> Vec<u8> {
    let mut bytes = original.to_vec();
    bytes[BITMAP..ENTRY].fill(0);
    let mut length = 0;
    for (position, value) in entries.iter().enumerate() {
        let shared = if position == 0 { 0 } else { prefix };
        assert_eq!(&entries[0][..shared], &value[..shared]);
        let value = &value[shared..];
        assert!(length + value.len() <= PAGE - ENTRY);
        bytes[ENTRY + length..ENTRY + length + value.len()].copy_from_slice(value);
        length += value.len();
        bytes[BITMAP + length / 8] |= 1 << (length % 8);
    }
    support::put16(&mut bytes, FREE, (PAGE - ENTRY - length) as u16);
    support::put16(&mut bytes, PREFIX, prefix as u16);
    bytes
}

pub fn fresh(entries: &[Vec<u8>], height: u8, prefix: usize) -> Vec<u8> {
    let mut initial = vec![0xa7; PAGE];
    initial[..ENTRY].fill(0);
    initial[..2].copy_from_slice(&[if height == 0 { 4 } else { 3 }, 1]);
    support::put32(&mut initial, 4, 10);
    initial[HEIGHT] = height;
    repack(&initial, entries, prefix)
}

pub fn map(numbers: &[usize]) -> Vec<u8> {
    let mut bytes = vec![0; MAP_BYTES];
    for &number in numbers {
        bytes[MAP_HEADER + number / 8] |= 1 << (number % 8);
    }
    bytes
}

pub fn source(
    code: u8,
    scores: Vec<Option<i32>>,
    counts: Vec<usize>,
    directions: Vec<bool>,
    bounded: bool,
) -> Model {
    assert_eq!(scores.len(), counts.iter().sum::<usize>());
    assert!(scores.len() <= DATA.len() * ROWS_PER_PAGE);
    let seed = root_model::database(
        code,
        (1..=500).map(Some).collect(),
        directions.clone(),
        true,
        false,
        false,
    );
    let spacing = counts.len().div_ceil(8) * 8;
    let leaves = (0..directions.len())
        .map(|physical| {
            (0..counts.len())
                .map(|offset| 32 + physical * spacing + offset)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let last = *leaves.last().unwrap().last().unwrap();
    let eof = (last + 1).div_ceil(8) * 8;
    assert!(eof < 512);
    let mut bytes = seed.bytes;
    bytes.resize(eof * PAGE, 0);
    support::put32(&mut bytes, 10 * PAGE + 16, scores.len() as u32);
    for physical in 0..directions.len() {
        support::put32(
            &mut bytes,
            10 * PAGE + 63 + physical * 12 + 4,
            scores.len() as u32,
        );
    }
    let mut data_pages = Vec::new();
    for (group, values) in scores.chunks(ROWS_PER_PAGE).enumerate() {
        let number = DATA[group];
        data_pages.push(number);
        let rows = values
            .iter()
            .enumerate()
            .map(|(slot, value)| {
                let row = group * ROWS_PER_PAGE + slot;
                let mut record = vec![2, 0];
                record.extend((row as i32 + 1).to_le_bytes());
                record.extend(value.unwrap_or(0).to_le_bytes());
                record.push(if value.is_some() { 3 } else { 1 });
                (0, record)
            })
            .collect::<Vec<_>>();
        let mut page = support::data_page(10, &rows);
        support::put16(
            &mut page,
            FREE,
            (PAGE - SLOT_START - rows.len() * 13) as u16,
        );
        bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(&page);
    }
    bytes[11 * PAGE..12 * PAGE].copy_from_slice(&support::data_page(0, &[(0, map(&data_pages))]));
    let future: Vec<_> = (eof..512).collect();
    bytes[PAGE..2 * PAGE].copy_from_slice(&support::data_page(1, &[(0, map(&future))]));
    for (physical, &descending) in directions.iter().enumerate() {
        let keys = ordered(&scores, descending);
        let mut offset = 0;
        let mut bounds = Vec::new();
        for (position, &count) in counts.iter().enumerate() {
            let number = leaves[physical][position];
            let entries = &keys[offset..offset + count];
            offset += count;
            let mut page = fresh(entries, 0, common(entries, 5));
            support::put32(
                &mut page,
                PREVIOUS,
                if position == 0 {
                    0
                } else {
                    leaves[physical][position - 1] as u32
                },
            );
            support::put32(
                &mut page,
                NEXT,
                leaves[physical].get(position + 1).copied().unwrap_or(0) as u32,
            );
            bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(&page);
            if bounded || position + 1 != counts.len() {
                let mut bound = entries.last().unwrap().clone();
                bound.extend((number as u32).to_be_bytes());
                bounds.push(bound);
            }
        }
        let mut root = fresh(&bounds, 1, common(&bounds, 5));
        if !bounded {
            support::put32(&mut root, TAIL, *leaves[physical].last().unwrap() as u32);
        }
        bytes[ROOTS[physical] * PAGE..(ROOTS[physical] + 1) * PAGE].copy_from_slice(&root);
        let mut owned = vec![ROOTS[physical]];
        owned.extend_from_slice(&leaves[physical]);
        bytes[MAPS[physical] * PAGE..(MAPS[physical] + 1) * PAGE]
            .copy_from_slice(&support::data_page(0, &[(0, map(&owned))]));
    }
    Model {
        bytes,
        scores,
        directions,
        leaves,
        counts,
        bounded,
        eof,
    }
}

pub fn groups(ties: bool) -> Vec<Option<i32>> {
    [0, 0x0100_0000, 0x0200_0000]
        .into_iter()
        .flat_map(|base| {
            (0..600).map(move |position| {
                Some(base + if ties { position / 2 + 1 } else { position + 1 })
            })
        })
        .collect()
}

pub fn check(model: &Model) {
    let result = check_table(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        result
            .indexes()
            .iter()
            .all(|index| index.status() == tablecraft_ace::IndexCheckStatus::Checked)
    );
}

pub fn run(
    model: &Model,
    request: Int32Update,
) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    root_model::capture(&model.bytes).preview_ace16_split_int32_updates(
        "Items",
        "Score",
        &[request],
        &mut budget(),
        &|| false,
    )
}

pub fn field_offset(bytes: &[u8], row: usize) -> usize {
    let number = DATA[row / ROWS_PER_PAGE];
    let slot = row % ROWS_PER_PAGE;
    let start = root_model::get16(bytes, number * PAGE + SLOT_START + 2 * slot) & OFFSET_MASK;
    number * PAGE + usize::from(start) + 6
}

pub fn decoded(bytes: &[u8], number: usize) -> Vec<Vec<u8>> {
    root_model::entries(bytes, number)
}

pub fn prefix_for(original: usize, entries: &[Vec<u8>]) -> Option<usize> {
    let best = common(entries, 5);
    let preserved = original.min(best);
    let size = |prefix| {
        entries.iter().map(Vec::len).sum::<usize>() - entries.len().saturating_sub(1) * prefix
    };
    if size(preserved) <= PAGE - ENTRY {
        Some(preserved)
    } else if size(best) <= PAGE - ENTRY {
        Some(best)
    } else {
        None
    }
}

pub struct SplitFact {
    pub physical: usize,
    pub old_right: usize,
    pub new_left: usize,
    pub left: Vec<Vec<u8>>,
    pub right: Vec<Vec<u8>>,
}

pub fn expected(model: &Model, row: usize, value: i32) -> (Vec<u8>, Vec<SplitFact>) {
    let mut after = model.bytes.clone();
    let at = field_offset(&model.bytes, row);
    after[at..at + 4].copy_from_slice(&value.to_le_bytes());
    let mut scores = model.scores.clone();
    scores[row] = Some(value);
    let mut facts = Vec::new();
    for (physical, &descending) in model.directions.iter().enumerate() {
        let keys = ordered(&scores, descending);
        let mut offset = 0;
        let mut leaves: Vec<(usize, Vec<Vec<u8>>, bool)> = Vec::new();
        for (position, &count) in model.counts.iter().enumerate() {
            let number = model.leaves[physical][position];
            let entries = keys[offset..offset + count].to_vec();
            offset += count;
            let original = &model.bytes[number * PAGE..(number + 1) * PAGE];
            let prefix = root_model::get16(original, PREFIX) as usize;
            if prefix_for(prefix, &entries).is_some() {
                leaves.push((number, entries, false));
            } else {
                let middle = entries.len() / 2;
                let new_left = after.len() / PAGE;
                after.extend(vec![0; PAGE]);
                let left = entries[..middle].to_vec();
                let right = entries[middle..].to_vec();
                facts.push(SplitFact {
                    physical,
                    old_right: number,
                    new_left,
                    left: left.clone(),
                    right: right.clone(),
                });
                leaves.push((new_left, left, true));
                leaves.push((number, right, false));
                for (map_page, set) in [(MAPS[physical], true), (1, false)] {
                    let byte = root_model::map_byte(&model.bytes, map_page, new_left);
                    let mask = 1 << (new_left % 8);
                    assert_eq!(model.bytes[byte] & mask != 0, !set);
                    if set {
                        after[byte] |= mask;
                    } else {
                        after[byte] &= !mask;
                    }
                }
            }
        }
        let mut bounds = Vec::new();
        for (position, (number, entries, fresh)) in leaves.iter().enumerate() {
            let mut initial = if *fresh {
                let mut bytes = vec![0; PAGE];
                bytes[..2].copy_from_slice(&[4, 1]);
                support::put32(&mut bytes, 4, 10);
                bytes
            } else {
                model.bytes[number * PAGE..(number + 1) * PAGE].to_vec()
            };
            let old_prefix = root_model::get16(&initial, PREFIX) as usize;
            let prefix = prefix_for(old_prefix, entries).unwrap();
            support::put32(
                &mut initial,
                PREVIOUS,
                position
                    .checked_sub(1)
                    .map(|index| leaves[index].0 as u32)
                    .unwrap_or(0),
            );
            support::put32(
                &mut initial,
                NEXT,
                leaves
                    .get(position + 1)
                    .map(|leaf| leaf.0 as u32)
                    .unwrap_or(0),
            );
            after[number * PAGE..(number + 1) * PAGE]
                .copy_from_slice(&repack(&initial, entries, prefix));
            if model.bounded || position + 1 != leaves.len() {
                let mut bound = entries.last().unwrap().clone();
                bound.extend((*number as u32).to_be_bytes());
                bounds.push(bound);
            }
        }
        let root = ROOTS[physical];
        let mut initial = model.bytes[root * PAGE..(root + 1) * PAGE].to_vec();
        let old_prefix = root_model::get16(&initial, PREFIX) as usize;
        let prefix = prefix_for(old_prefix, &bounds).unwrap();
        support::put32(
            &mut initial,
            TAIL,
            if model.bounded {
                0
            } else {
                leaves.last().unwrap().0 as u32
            },
        );
        after[root * PAGE..(root + 1) * PAGE].copy_from_slice(&repack(&initial, &bounds, prefix));
    }
    (after, facts)
}

pub fn verify_values(model: &Model, bytes: &[u8], row: usize, value: i32) {
    let result = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(result.rows().len(), model.scores.len());
    for (position, actual) in result.rows().iter().enumerate() {
        let score = if position == row {
            Some(value)
        } else {
            model.scores[position]
        };
        assert_eq!(actual[0], Value::Int32(position as i32 + 1));
        assert_eq!(actual[1], score.map(Value::Int32).unwrap_or(Value::Null));
    }
}
