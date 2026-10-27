use super::*;

pub const PAGE: usize = 4096;
pub const ENTRY: usize = 480;
pub const BITMAP: usize = 27;
pub const PREFIX: usize = 24;
pub const HEIGHT: usize = 26;
pub const PREVIOUS: usize = 12;
pub const NEXT: usize = 16;
pub const TAIL: usize = 20;
pub const ROOTS: [usize; 2] = [13, 19];
pub const MAPS: [usize; 2] = [14, 20];
pub const DATA: [usize; 8] = [12, 15, 16, 17, 18, 21, 22, 23];
pub const PER_PAGE: usize = 200;
const TABLE: usize = 10;
const MAP_HEADER: usize = 5;
const MAP_BYTES: usize = 261;
const MAP_COVERAGE: usize = 2048;
const FIELD_HEADER: usize = 63;
const INDEX_COUNTER: usize = 12;
const COLUMN_BYTES: usize = 25;
const FIELD_LENGTH: usize = 23;
const ROW_FIELD: usize = 6;
const SLOT_START: usize = 14;
const SLOT_MASK: u16 = 0x1fff;

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Byte,
    Int16,
    Int32,
    Int64,
    Currency,
}

impl Kind {
    pub fn width(self) -> usize {
        match self {
            Self::Byte => 1,
            Self::Int16 => 2,
            Self::Int32 => 4,
            Self::Int64 | Self::Currency => 8,
        }
    }

    fn native(self) -> u8 {
        match self {
            Self::Byte => 2,
            Self::Int16 => 3,
            Self::Int32 => 4,
            Self::Int64 => 0x13,
            Self::Currency => 5,
        }
    }

    pub fn maximum(self) -> i64 {
        match self {
            Self::Byte => 255,
            Self::Int16 => i16::MAX as i64,
            Self::Int32 => i32::MAX as i64,
            Self::Int64 | Self::Currency => i64::MAX,
        }
    }

    pub fn value(self, units: i64) -> Value {
        match self {
            Self::Byte => Value::Byte(u8::try_from(units).unwrap()),
            Self::Int16 => Value::Int16(i16::try_from(units).unwrap()),
            Self::Int32 => Value::Int32(i32::try_from(units).unwrap()),
            Self::Int64 => Value::Int64(units),
            Self::Currency => Value::Currency(Currency::from_scaled(units)),
        }
    }

    fn stored(self, units: i64) -> Vec<u8> {
        let bytes = units.to_le_bytes();
        bytes[..self.width()].to_vec()
    }

    pub fn root_count(self) -> usize {
        if self.width() == 8 { 400 } else { 900 }
    }
}

pub struct Model {
    pub bytes: Vec<u8>,
    pub values: Vec<Option<i64>>,
    pub kind: Kind,
    pub leaves: Vec<Vec<usize>>,
    pub counts: Vec<usize>,
    pub eof: usize,
    pub root_leaf: bool,
    pub replacement: i64,
}

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

pub fn reference(row: usize) -> u32 {
    ((DATA[row / PER_PAGE] as u32) << 8) | (row % PER_PAGE) as u32
}

pub fn wire(kind: Kind, row: usize, value: Option<i64>, descending: bool) -> Vec<u8> {
    let mut result = Vec::new();
    if let Some(value) = value {
        result.push(if descending { 128 } else { 127 });
        let width = kind.width();
        let bytes = value.to_be_bytes();
        let mut encoded = bytes[bytes.len() - width..].to_vec();
        if !matches!(kind, Kind::Byte) {
            encoded[0] ^= 128;
        }
        if descending {
            for byte in &mut encoded {
                *byte = !*byte;
            }
        }
        result.extend(encoded);
    } else {
        result.push(if descending { 255 } else { 0 });
    }
    result.extend(reference(row).to_be_bytes());
    result
}

pub fn ordered(model: &Model, values: &[Option<i64>], descending: bool) -> Vec<Vec<u8>> {
    let mut result = values
        .iter()
        .enumerate()
        .map(|(row, &value)| wire(model.kind, row, value, descending))
        .collect::<Vec<_>>();
    result.sort();
    result
}

pub fn common(entries: &[Vec<u8>], child: bool) -> usize {
    if entries.len() < 2 {
        return 0;
    }
    let first = &entries[0];
    let mut prefix = first.len() - if child { 4 } else { 0 };
    for entry in &entries[1..] {
        prefix = prefix.min(entry.len() - if child { 4 } else { 0 });
        prefix = first[..prefix]
            .iter()
            .zip(&entry[..prefix])
            .take_while(|(left, right)| left == right)
            .count();
    }
    prefix
}

pub fn encoded_len(entries: &[Vec<u8>], prefix: usize) -> usize {
    entries.iter().map(Vec::len).sum::<usize>() - prefix * (entries.len() - 1)
}

pub fn blank(height: u8, fresh: bool) -> Vec<u8> {
    let mut result = vec![if fresh { 0 } else { 0xa7 }; PAGE];
    result[..ENTRY].fill(0);
    result[..2].copy_from_slice(&[if height == 0 { 4 } else { 3 }, 1]);
    support::put32(&mut result, 4, TABLE as u32);
    result[HEIGHT] = height;
    result
}

pub fn repack(original: &[u8], entries: &[Vec<u8>], prefix: usize) -> Vec<u8> {
    let mut result = original.to_vec();
    result[BITMAP..ENTRY].fill(0);
    support::put16(&mut result, PREFIX, prefix as u16);
    let mut length = 0;
    for (position, entry) in entries.iter().enumerate() {
        let shared = if position == 0 { 0 } else { prefix };
        assert_eq!(entry[..shared], entries[0][..shared]);
        let suffix = &entry[shared..];
        assert!(length + suffix.len() <= PAGE - ENTRY);
        result[ENTRY + length..ENTRY + length + suffix.len()].copy_from_slice(suffix);
        length += suffix.len();
        result[BITMAP + length / 8] |= 1 << (length % 8);
    }
    support::put16(&mut result, 2, (PAGE - ENTRY - length) as u16);
    result
}

pub fn bound(key: &[u8], child: usize) -> Vec<u8> {
    let mut result = key.to_vec();
    result.extend((child as u32).to_be_bytes());
    result
}

pub fn install(bytes: &mut [u8], page: usize, value: &[u8]) {
    bytes[page * PAGE..(page + 1) * PAGE].copy_from_slice(value);
}

fn usage(pages: &[usize]) -> Vec<u8> {
    let mut result = vec![0; MAP_BYTES];
    for &page in pages {
        result[MAP_HEADER + page / 8] |= 1 << (page % 8);
    }
    result
}

pub fn source(code: u8, kind: Kind, counts: Vec<usize>, null_peer: bool) -> Model {
    let root_leaf = counts.len() == 1;
    let mut values = vec![Some(0); counts.iter().sum()];
    if null_peer {
        *values.last_mut().unwrap() = None;
    }
    assert!(values.len() <= DATA.len() * PER_PAGE);
    let seed = root_model::database(
        code,
        vec![Some(0); 10],
        vec![false, true],
        true,
        false,
        false,
    );
    let mut bytes = seed.bytes;
    let leaves = (0..2)
        .map(|physical| {
            if root_leaf {
                vec![ROOTS[physical]]
            } else {
                (0..counts.len())
                    .map(|position| 32 + physical * counts.len() + position)
                    .collect()
            }
        })
        .collect::<Vec<Vec<_>>>();
    let eof = if root_leaf {
        24
    } else {
        (32 + 2 * counts.len()).div_ceil(8) * 8
    };
    assert!(eof + 8 < MAP_COVERAGE);
    bytes.resize(eof * PAGE, 0);
    support::put32(&mut bytes, TABLE * PAGE + 16, values.len() as u32);
    for physical in 0..2 {
        support::put32(
            &mut bytes,
            TABLE * PAGE + FIELD_HEADER + physical * INDEX_COUNTER + 4,
            values.len() as u32,
        );
    }
    let selected = TABLE * PAGE + FIELD_HEADER + 2 * INDEX_COUNTER + COLUMN_BYTES;
    bytes[selected] = kind.native();
    support::put16(&mut bytes, selected + FIELD_LENGTH, kind.width() as u16);
    let mut data_pages = Vec::new();
    for (group, chunk) in values.chunks(PER_PAGE).enumerate() {
        let page = DATA[group];
        data_pages.push(page);
        let rows = chunk
            .iter()
            .enumerate()
            .map(|(slot, &value)| {
                let mut record = vec![2, 0];
                record.extend((group as i32 * PER_PAGE as i32 + slot as i32 + 1).to_le_bytes());
                record.extend(kind.stored(value.unwrap_or(0)));
                record.push(if value.is_some() { 3 } else { 1 });
                (0, record)
            })
            .collect::<Vec<_>>();
        let used = rows.iter().map(|(_, row)| row.len() + 2).sum::<usize>();
        let mut data = support::data_page(TABLE as u32, &rows);
        support::put16(&mut data, 2, (PAGE - SLOT_START - used) as u16);
        install(&mut bytes, page, &data);
    }
    install(
        &mut bytes,
        11,
        &support::data_page(0, &[(0, usage(&data_pages))]),
    );
    let mut global = vec![0; MAP_BYTES];
    for page in eof..MAP_COVERAGE {
        global[MAP_HEADER + page / 8] |= 1 << (page % 8);
    }
    install(&mut bytes, 1, &support::data_page(1, &[(0, global)]));
    let model = Model {
        bytes,
        values,
        kind,
        leaves,
        counts,
        eof,
        root_leaf,
        replacement: kind.maximum(),
    };
    initialize_indexes(model)
}

fn counts_for(model: &Model, physical: usize) -> Vec<usize> {
    let mut counts = model.counts.clone();
    if physical == 0 {
        counts.reverse();
    }
    counts
}

fn initialize_indexes(mut model: Model) -> Model {
    for physical in 0..2 {
        let keys = ordered(&model, &model.values, physical == 1);
        let mut offset = 0;
        let mut bounds = Vec::new();
        for (position, &count) in counts_for(&model, physical).iter().enumerate() {
            let entries = &keys[offset..offset + count];
            offset += count;
            let number = model.leaves[physical][position];
            let mut page = repack(&blank(0, false), entries, common(entries, false));
            support::put32(
                &mut page,
                PREVIOUS,
                if position == 0 {
                    0
                } else {
                    model.leaves[physical][position - 1] as u32
                },
            );
            support::put32(
                &mut page,
                NEXT,
                model.leaves[physical]
                    .get(position + 1)
                    .copied()
                    .unwrap_or(0) as u32,
            );
            install(&mut model.bytes, number, &page);
            bounds.push(bound(entries.last().unwrap(), number));
        }
        let mut owned = model.leaves[physical].clone();
        if !model.root_leaf {
            let root = repack(&blank(1, false), &bounds, common(&bounds, true));
            install(&mut model.bytes, ROOTS[physical], &root);
            owned.push(ROOTS[physical]);
        }
        install(
            &mut model.bytes,
            MAPS[physical],
            &support::data_page(0, &[(0, usage(&owned))]),
        );
    }
    model
}

pub fn field_offset(bytes: &[u8], row: usize) -> usize {
    let page = DATA[row / PER_PAGE];
    let start =
        root_model::get16(bytes, page * PAGE + SLOT_START + 2 * (row % PER_PAGE)) & SLOT_MASK;
    page * PAGE + start as usize + ROW_FIELD
}

pub fn request(model: &Model) -> FixedScalarUpdate {
    FixedScalarUpdate::new(0, model.kind.value(0), model.kind.value(model.replacement))
}

fn adaptive(original: &[u8], entries: &[Vec<u8>], child: bool) -> Vec<u8> {
    let previous = root_model::get16(original, PREFIX) as usize;
    let best = common(entries, child);
    let mut prefix = previous.min(best);
    if encoded_len(entries, prefix) > PAGE - ENTRY {
        prefix = best;
    }
    repack(original, entries, prefix)
}

fn allocate(after: &mut Vec<u8>, allocated: &mut Vec<usize>, page: Vec<u8>) -> usize {
    let number = after.len() / PAGE;
    after.extend(page);
    allocated.push(number);
    number
}

fn set_bit(bytes: &mut [u8], map: usize, number: usize, set: bool) {
    let offset = root_model::map_byte(bytes, map, number);
    let mask = 1 << (number % 8);
    if set {
        bytes[offset] |= mask;
    } else {
        bytes[offset] &= !mask;
    }
}

pub fn expected(model: &Model) -> Vec<u8> {
    let mut after = model.bytes.clone();
    let field = field_offset(&after, 0);
    let replacement = model.kind.stored(model.replacement);
    after[field..field + replacement.len()].copy_from_slice(&replacement);
    let mut values = model.values.clone();
    values[0] = Some(model.replacement);
    for physical in 0..2 {
        let mut allocated = Vec::new();
        let keys = ordered(model, &values, physical == 1);
        let root = ROOTS[physical];
        let original_root = &model.bytes[root * PAGE..(root + 1) * PAGE];
        if model.root_leaf {
            let best = common(&keys, false);
            if encoded_len(&keys, best) <= PAGE - ENTRY {
                let page = adaptive(original_root, &keys, false);
                install(&mut after, root, &page);
                continue;
            }
            let middle = keys.len() / 2;
            let right_page = adaptive(&blank(0, true), &keys[middle..], false);
            let right = allocate(&mut after, &mut allocated, right_page);
            let left_page = adaptive(&blank(0, true), &keys[..middle], false);
            let left = allocate(&mut after, &mut allocated, left_page);
            support::put32(&mut after, right * PAGE + PREVIOUS, left as u32);
            support::put32(&mut after, left * PAGE + NEXT, right as u32);
            let mut initial = original_root.to_vec();
            initial[..2].copy_from_slice(&[3, 1]);
            initial[HEIGHT] = 1;
            support::put32(&mut initial, TAIL, right as u32);
            let root_page = repack(&initial, &[bound(&keys[middle - 1], left)], 0);
            install(&mut after, root, &root_page);
        } else {
            let mut children = Vec::new();
            let mut offset = 0;
            for (position, &count) in counts_for(model, physical).iter().enumerate() {
                let number = model.leaves[physical][position];
                let entries = &keys[offset..offset + count];
                offset += count;
                let original = &model.bytes[number * PAGE..(number + 1) * PAGE];
                if encoded_len(entries, common(entries, false)) > PAGE - ENTRY {
                    let middle = count / 2;
                    let left_page = adaptive(&blank(0, true), &entries[..middle], false);
                    let left = allocate(&mut after, &mut allocated, left_page);
                    children.push((left, entries[middle - 1].clone()));
                    let right_page = adaptive(original, &entries[middle..], false);
                    install(&mut after, number, &right_page);
                } else {
                    let page = adaptive(original, entries, false);
                    install(&mut after, number, &page);
                }
                children.push((number, entries.last().unwrap().clone()));
            }
            for (position, (number, _)) in children.iter().enumerate() {
                let previous = if position == 0 {
                    0
                } else {
                    children[position - 1].0
                };
                let next = children.get(position + 1).map(|child| child.0).unwrap_or(0);
                support::put32(&mut after, number * PAGE + PREVIOUS, previous as u32);
                support::put32(&mut after, number * PAGE + NEXT, next as u32);
            }
            let bounds = children
                .iter()
                .map(|(number, key)| bound(key, *number))
                .collect::<Vec<_>>();
            if encoded_len(&bounds, common(&bounds, true)) <= PAGE - ENTRY {
                let page = adaptive(original_root, &bounds, true);
                install(&mut after, root, &page);
            } else {
                let middle = children.len() / 2;
                let right_bounds = &bounds[middle..bounds.len() - 1];
                let mut right_page = adaptive(&blank(1, true), right_bounds, true);
                support::put32(&mut right_page, TAIL, children.last().unwrap().0 as u32);
                let right = allocate(&mut after, &mut allocated, right_page);
                let left_bounds = &bounds[..middle - 1];
                let mut left_page = adaptive(&blank(1, true), left_bounds, true);
                support::put32(&mut left_page, TAIL, children[middle - 1].0 as u32);
                let left = allocate(&mut after, &mut allocated, left_page);
                support::put32(&mut after, right * PAGE + PREVIOUS, left as u32);
                support::put32(&mut after, left * PAGE + NEXT, right as u32);
                let root_bounds = vec![
                    bound(&children[middle - 1].1, left),
                    bound(&children.last().unwrap().1, right),
                ];
                let mut initial = original_root.to_vec();
                initial[HEIGHT] = 2;
                let page = repack(&initial, &root_bounds, 0);
                install(&mut after, root, &page);
            }
        }
        for number in allocated {
            set_bit(&mut after, MAPS[physical], number, true);
            set_bit(&mut after, 1, number, false);
        }
    }
    after
}

pub fn check(model: &Model) {
    let report = check_table(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.indexes().len(), 4);
    assert!(
        report
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let table = read_table(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows().len(), model.values.len());
    for (row, expected) in table.rows().iter().zip(&model.values) {
        let value = expected
            .map(|units| model.kind.value(units))
            .unwrap_or(Value::Null);
        assert_eq!(row[1], value);
    }
}
