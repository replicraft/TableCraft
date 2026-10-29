//! Source/request-only ordinary ownership promotion after a prior literal image.
use crate::table::mutation::insert::tests::global_map_growth::model::{self as global, Phase};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const SOURCE_EOF: usize = 520;
const FIRST_DATA: u32 = 520;
const OWNED_BITMAP: u32 = 521;
const SECOND_DATA: u32 = 522;

pub(super) struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub old_rows: Vec<Vec<Value>>,
    pub final_rows: Vec<Vec<Value>>,
    pub values: Vec<Vec<Value>>,
    pub data: Vec<u32>,
    pub owned: Vec<u32>,
}

fn row_start(bytes: &[u8], page: usize, slot: usize) -> usize {
    let at = page * PAGE + 14 + slot * 2;
    let word = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    assert_eq!(word & 0xe000, 0);
    page * PAGE + usize::from(word)
}

fn new_row(id: i32) -> Vec<u8> {
    let mut row = vec![2, 0];
    row.extend(id.to_le_bytes());
    row.extend([0xe7; 255]);
    assert_eq!(row.len(), 261);
    row.extend([5, 1, 6, 0, 1, 0, 3]);
    assert_eq!(row.len(), 268);
    row
}

fn data_page(rows: &[Vec<u8>]) -> Vec<u8> {
    let mut page = vec![0; PAGE];
    page[..2].copy_from_slice(&[1, 1]);
    page[4..8].copy_from_slice(&10_u32.to_le_bytes());
    page[12..14].copy_from_slice(&(rows.len() as u16).to_le_bytes());
    let mut end = PAGE;
    for (slot, row) in rows.iter().enumerate() {
        let start = end - row.len();
        page[start..end].copy_from_slice(row);
        page[14 + slot * 2..16 + slot * 2].copy_from_slice(&(start as u16).to_le_bytes());
        end = start;
    }
    let free = end - 14 - rows.len() * 2;
    page[2..4].copy_from_slice(&(free as u16).to_le_bytes());
    page
}

pub(super) fn case(code: u8, count: usize) -> Case {
    assert!(matches!(count, 1 | 16));
    // The donor is a previously qualified independent literal image, not a
    // producer output. Its 452 visible rows and native graph remain real.
    let original = global::case(code, Phase::Data, 512, false);
    let old_rows = original.final_rows;
    assert_eq!(old_rows.len(), 452);
    let mut before = original.expected;
    assert_eq!(before.len(), 516 * PAGE);
    assert_eq!(
        &before[10 * PAGE + 16..10 * PAGE + 20],
        &452_u32.to_le_bytes()
    );
    assert_eq!(
        &before[10 * PAGE + 67..10 * PAGE + 71],
        &452_u32.to_le_bytes()
    );
    before.resize(SOURCE_EOF * PAGE, 0);
    for page in 516..SOURCE_EOF {
        before[page * PAGE..(page + 1) * PAGE].fill(0x5a);
    }
    // The new opaque allocated pages do not become ordinary DATA members.
    assert_eq!(before[513 * PAGE + 68], 240);
    before[513 * PAGE + 68] = 0;

    // Preserve visible row512:0 and append one genuine deleted filler slot.
    let highest = 512 * PAGE;
    assert_eq!(
        u16::from_le_bytes(before[highest + 14..highest + 16].try_into().unwrap()),
        3999
    );
    before[highest + 19..highest + 3999].fill(0x6d);
    before[highest + 16..highest + 18].copy_from_slice(&(0x8000_u16 | 19).to_le_bytes());
    before[highest + 12..highest + 14].copy_from_slice(&2_u16.to_le_bytes());
    before[highest + 2..highest + 4].copy_from_slice(&1_u16.to_le_bytes());

    let values = (0..count)
        .map(|position| {
            vec![
                Value::Int32(227 + position as i32),
                Value::Binary(vec![0xe7; 255]),
            ]
        })
        .collect::<Vec<_>>();
    let records = (0..count)
        .map(|position| new_row(227 + position as i32))
        .collect::<Vec<_>>();
    let data = if count == 1 {
        vec![FIRST_DATA]
    } else {
        vec![FIRST_DATA, SECOND_DATA]
    };
    let end = if count == 1 { 522 } else { 523 };
    let mut expected = before.clone();
    expected.resize(end * PAGE, 0);
    for (position, subset) in records.chunks(15).enumerate() {
        let number = data[position] as usize;
        expected[number * PAGE..(number + 1) * PAGE].copy_from_slice(&data_page(subset));
    }

    let mut owned = vec![12];
    owned.extend(15..25);
    owned.push(512);
    owned.extend(data.iter().copied());
    let mut bitmap = vec![0; PAGE];
    bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
    for &member in &owned {
        bitmap[4 + member as usize / 8] |= 1 << (member % 8);
    }
    expected[OWNED_BITMAP as usize * PAGE..(OWNED_BITMAP as usize + 1) * PAGE]
        .copy_from_slice(&bitmap);
    let owner = row_start(&before, 11, 0);
    assert_eq!(&before[owner..owner + 5], &[0, 8, 0, 0, 0]);
    expected[owner..owner + 69].fill(0);
    expected[owner] = 1;
    expected[owner + 1..owner + 5].copy_from_slice(&OWNED_BITMAP.to_le_bytes());
    let hint = row_start(&before, 11, 1);
    assert_eq!(&before[hint..hint + 6], &[0, 0, 2, 0, 0, 1]);
    expected[hint + 5..hint + 69].fill(0);
    let last = *data.last().unwrap();
    expected[hint + 5 + (last as usize - 512) / 8] |= 1 << ((last - 512) % 8);
    for page in FIRST_DATA..end as u32 {
        expected[513 * PAGE + 4 + page as usize / 8] &= !(1 << (page % 8));
    }

    // The right leaf fits all new entries without changing its retained prefix,
    // root, sibling links or unused suffix. The original active stream has 226
    // complete nine-byte ID+ROW entries and no compressed bytes.
    let right = 514 * PAGE;
    assert_eq!(&before[right..right + 4], &[4, 1, 46, 6]);
    assert_eq!(&before[right + 24..right + 26], &[0, 0]);
    let mut active = 226 * 9;
    for position in 0..count {
        let id = 227 + position as i32;
        let page = data[position / 15];
        let reference = (page << 8) | (position % 15) as u32;
        let mut key = vec![127];
        key.extend(((id as u32) ^ 0x80000000).to_be_bytes());
        key.extend(reference.to_be_bytes());
        expected[right + 480 + active..right + 480 + active + 9].copy_from_slice(&key);
        active += 9;
        expected[right + 27 + active / 8] |= 1 << (active % 8);
    }
    expected[right + 2..right + 4].copy_from_slice(&((3616 - active) as u16).to_le_bytes());
    let rows = 452 + count as u32;
    expected[10 * PAGE + 16..10 * PAGE + 20].copy_from_slice(&rows.to_le_bytes());
    expected[10 * PAGE + 67..10 * PAGE + 71].copy_from_slice(&rows.to_le_bytes());
    let mut final_rows = old_rows.clone();
    final_rows.extend(values.iter().cloned());
    Case {
        before,
        expected,
        old_rows,
        final_rows,
        values,
        data,
        owned,
    }
}
