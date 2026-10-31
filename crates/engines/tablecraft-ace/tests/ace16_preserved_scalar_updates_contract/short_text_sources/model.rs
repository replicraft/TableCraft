//! Independent fitting expectations from the unchanged rich original owner.
use super::rich;

const PAGE_BYTES: usize = 4096;
const DEFINITION_PAGE: usize = 10;
const DATA_PAGE: usize = 12;
const COLUMN_BYTES: usize = 25;
const FIRST_COLUMN: usize = 63;
const COUNTER_BYTES: usize = 12;
const TARGET_NUMBER: usize = 1;
const HEADER_BYTES: usize = 14;
const SLOT_BYTES: usize = 2;
const COUNT_BYTES: usize = 2;
const MASK_BITS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Recipe {
    ShorterNarrow,
    ShorterUnicode,
    SameCapacityUnicode,
    NullToValue,
    ValueToNull,
    Empty,
    Noop,
    NullNoop,
    Overflow,
}

pub(crate) const RECIPES: [Recipe; 9] = [
    Recipe::ShorterNarrow,
    Recipe::ShorterUnicode,
    Recipe::SameCapacityUnicode,
    Recipe::NullToValue,
    Recipe::ValueToNull,
    Recipe::Empty,
    Recipe::Noop,
    Recipe::NullNoop,
    Recipe::Overflow,
];

pub(crate) struct Framing {
    pub row: usize,
    pub absolute: usize,
    pub length: usize,
    pub count: usize,
    pub mask_at: usize,
    pub count_at: usize,
    pub data_end: usize,
    pub offsets: Vec<usize>,
    pub variable: usize,
    pub capacity: usize,
    pub present: bool,
}

pub(crate) struct Case {
    pub before: Vec<u8>,
    pub expected: Option<Vec<u8>>,
    pub framing: Framing,
    pub original: Option<&'static str>,
    pub replacement: Option<&'static str>,
    pub payload: Option<&'static [u8]>,
    pub admitted_source: bool,
}

fn u16_at(bytes: &[u8], offset: usize) -> usize {
    usize::from(u16::from_le_bytes(
        bytes[offset..offset + 2].try_into().unwrap(),
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> usize {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
}

fn framing(before: &[u8], row: usize) -> Framing {
    let definition = DEFINITION_PAGE * PAGE_BYTES;
    let fields = u16_at(before, definition + 45);
    let logical = u32_at(before, definition + 47);
    let physical = u32_at(before, definition + 51);
    assert_eq!((fields, logical, physical), (16, 3, 3));
    let descriptors = definition + FIRST_COLUMN + physical * COUNTER_BYTES;
    let mut target = None;
    for column in 0..fields {
        let at = descriptors + column * COLUMN_BYTES;
        if u16_at(before, at + 5) == TARGET_NUMBER {
            assert!(target.replace(at).is_none());
        }
    }
    let target = target.unwrap();
    assert_eq!(before[target], 10);
    assert_eq!(&before[target + 15..target + 17], &[2, 1]);
    assert_eq!(u16_at(before, target + 23), 510);
    let variable = u16_at(before, target + 7);
    assert_eq!(variable, 0);

    let page = DATA_PAGE * PAGE_BYTES;
    assert_eq!(&before[page..page + 2], &[1, 1]);
    assert_eq!(u32_at(before, page + 4), DEFINITION_PAGE);
    assert_eq!(u16_at(before, page + 12), 3);
    let slot = page + HEADER_BYTES + row * SLOT_BYTES;
    let start = u16_at(before, slot);
    let end = if row == 0 {
        PAGE_BYTES
    } else {
        u16_at(before, slot - SLOT_BYTES)
    };
    assert_eq!(start & 0xe000, 0);
    assert!(start < end && end <= PAGE_BYTES);
    let absolute = page + start;
    let record = &before[absolute..page + end];
    let count = u16_at(record, 0);
    assert_eq!(count, fields);
    let mask_at = record.len() - count.div_ceil(MASK_BITS);
    let count_at = mask_at - COUNT_BYTES;
    let variables = u16_at(record, count_at);
    assert_eq!(variables, 5);
    let data_end = count_at - (variables + 1) * SLOT_BYTES;
    let offsets = (0..=variables)
        .map(|position| u16_at(record, count_at - (position + 1) * SLOT_BYTES))
        .collect::<Vec<_>>();
    assert_eq!(offsets[0], 130);
    assert!(offsets.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(*offsets.last().unwrap() <= data_end);
    let present =
        record[mask_at + TARGET_NUMBER / MASK_BITS] & (1 << (TARGET_NUMBER % MASK_BITS)) != 0;
    let start = offsets[variable];
    let stop = offsets[variable + 1];
    let preserved = offsets.last().unwrap() - (stop - start);
    let capacity = data_end - preserved;
    assert_eq!(capacity, if row == 1 { 2 } else { 10 });
    if row == 1 {
        assert!(!present);
        assert_eq!(&record[start..stop], &[0xd1, 0xe2]);
    } else {
        assert!(present);
        assert_eq!(
            &record[start..stop],
            &[97, 0, 108, 0, 112, 0, 104, 0, 97, 0]
        );
    }
    Framing {
        row,
        absolute,
        length: record.len(),
        count,
        mask_at,
        count_at,
        data_end,
        offsets,
        variable,
        capacity,
        present,
    }
}

pub(crate) fn source(code: u8, recipe: Recipe) -> Case {
    assert!(matches!(code, 5 | 6));
    let mut before = rich::source().before;
    assert_eq!(before[20], 6);
    before[20] = code;
    let (row, original, replacement, payload) = match recipe {
        Recipe::ShorterNarrow => (
            0,
            Some("alpha"),
            Some("beta"),
            Some(&[0xff, 0xfe, b'b', b'e', b't', b'a'][..]),
        ),
        Recipe::ShorterUnicode => (0, Some("alpha"), Some("雪"), Some(&[0xea, 0x96][..])),
        Recipe::SameCapacityUnicode => (
            0,
            Some("alpha"),
            Some("Ω雪🙂a"),
            Some(&[0xa9, 0x03, 0xea, 0x96, 0x3d, 0xd8, 0x42, 0xde, 0x61, 0x00][..]),
        ),
        Recipe::NullToValue => (1, None, Some("é"), Some(&[0xe9, 0x00][..])),
        Recipe::ValueToNull => (0, Some("alpha"), None, Some(&[][..])),
        Recipe::Empty => (0, Some("alpha"), Some(""), Some(&[0xff, 0xfe][..])),
        Recipe::Noop => (
            0,
            Some("alpha"),
            Some("alpha"),
            Some(&[97, 0, 108, 0, 112, 0, 104, 0, 97, 0][..]),
        ),
        Recipe::NullNoop => (1, None, None, Some(&[0xd1, 0xe2][..])),
        Recipe::Overflow => (0, Some("alpha"), Some("雪雪雪雪雪雪"), None),
    };
    let framing = framing(&before, row);
    let expected = payload.map(|payload| {
        if matches!(recipe, Recipe::Noop | Recipe::NullNoop) {
            return before.clone();
        }
        assert!(payload.len() <= framing.capacity);
        let mut after = before.clone();
        let original_record = &before[framing.absolute..framing.absolute + framing.length];
        let record = &mut after[framing.absolute..framing.absolute + framing.length];
        let selected_start = framing.offsets[framing.variable];
        let selected_stop = framing.offsets[framing.variable + 1];
        let original_end = *framing.offsets.last().unwrap();
        let selected_end = selected_start + payload.len();
        let final_end = selected_end + original_end - selected_stop;
        assert!(final_end <= framing.data_end);
        record[selected_start..selected_end].copy_from_slice(payload);
        record[selected_end..final_end]
            .copy_from_slice(&original_record[selected_stop..original_end]);
        for (position, original_offset) in framing.offsets.iter().enumerate() {
            let final_offset = if position <= framing.variable {
                *original_offset
            } else {
                selected_end + original_offset - selected_stop
            };
            let at = framing.count_at - (position + 1) * SLOT_BYTES;
            record[at..at + SLOT_BYTES]
                .copy_from_slice(&u16::try_from(final_offset).unwrap().to_le_bytes());
        }
        let mask = &mut record[framing.mask_at + TARGET_NUMBER / MASK_BITS];
        let bit = 1 << (TARGET_NUMBER % MASK_BITS);
        *mask = (*mask & !bit) | if replacement.is_some() { bit } else { 0 };
        assert_eq!(
            &record[..selected_start],
            &original_record[..selected_start]
        );
        for variable in 1..framing.offsets.len() - 1 {
            let start = framing.offsets[variable];
            let stop = framing.offsets[variable + 1];
            let relocated = selected_end + start - selected_stop;
            assert_eq!(
                &record[relocated..relocated + stop - start],
                &original_record[start..stop]
            );
        }
        assert_eq!(
            record[framing.mask_at] & !bit,
            original_record[framing.mask_at] & !bit
        );
        assert_eq!(
            &record[framing.mask_at + 1..],
            &original_record[framing.mask_at + 1..]
        );
        assert_eq!(&after[..framing.absolute], &before[..framing.absolute]);
        assert_eq!(
            &after[framing.absolute + framing.length..],
            &before[framing.absolute + framing.length..]
        );
        after
    });
    Case {
        before,
        expected,
        framing,
        original,
        replacement,
        payload,
        admitted_source: code == 6,
    }
}
