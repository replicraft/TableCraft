//! Independent fitting Text reconstruction followed by literal fixed overlays.
use super::{fixed_model, text_model, zero_length_source};
use fixed_model::{
    Cell::{self, Boolean as B, Currency as C, Date as D, Int32 as I, Int64 as L, Null as N},
    Column::{self, Currency as Money, Date, Large, LargeInt32, Yes},
    Flavor, Group,
};
use std::collections::BTreeSet;
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const DATA_PAGE: usize = 12;
const DEFINITION_PAGE: usize = 10;
const TEXT_NUMBER: usize = 1;
const EMPTY: &[u8] = &[0xff, 0xfe];

#[derive(Clone, Copy, Debug)]
pub(crate) struct Text {
    pub row: usize,
    pub expected: Option<&'static str>,
    pub replacement: Option<&'static str>,
    pub payload: Option<&'static [u8]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Recipe {
    BetaLarge,
    UnicodeCurrency,
    SameCapacityDate,
    BetaBoolean,
    RawTextNoopFixed,
    NullTextNoopFixed,
    TextToNullMasks,
    NullTextToEmptyMasks,
    TextChangeInactiveDateNoop,
    DisjointRows,
    Int32TextChange,
    Int32TextNoopPadding,
    Int32NullPaddingText,
    AllNoops,
    EmptyTextFixedChange,
    FixedEmptyTextChange,
    EntirelyEmpty,
    BothInactiveNoopsWithBoolean,
    OverflowAfterFixed,
    DisallowEmptyText,
    TextRequired,
    NumericRequired,
    LateStaleText,
    LateStaleFixed,
    WrongFixedKind,
    EmptyIndexedFixed,
    DuplicateTextRow,
}

pub(crate) const RECIPES: [Recipe; 27] = [
    Recipe::BetaLarge,
    Recipe::UnicodeCurrency,
    Recipe::SameCapacityDate,
    Recipe::BetaBoolean,
    Recipe::RawTextNoopFixed,
    Recipe::NullTextNoopFixed,
    Recipe::TextToNullMasks,
    Recipe::NullTextToEmptyMasks,
    Recipe::TextChangeInactiveDateNoop,
    Recipe::DisjointRows,
    Recipe::Int32TextChange,
    Recipe::Int32TextNoopPadding,
    Recipe::Int32NullPaddingText,
    Recipe::AllNoops,
    Recipe::EmptyTextFixedChange,
    Recipe::FixedEmptyTextChange,
    Recipe::EntirelyEmpty,
    Recipe::BothInactiveNoopsWithBoolean,
    Recipe::OverflowAfterFixed,
    Recipe::DisallowEmptyText,
    Recipe::TextRequired,
    Recipe::NumericRequired,
    Recipe::LateStaleText,
    Recipe::LateStaleFixed,
    Recipe::WrongFixedKind,
    Recipe::EmptyIndexedFixed,
    Recipe::DuplicateTextRow,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Refusal {
    RowLayout,
    EmptyText,
    Required,
    Conflict,
    Column,
    IndexedColumn,
}

pub(crate) struct Case {
    pub before: Vec<u8>,
    pub expected: Option<Vec<u8>>,
    pub texts: Vec<Text>,
    pub fixed: Vec<Group>,
    pub flavor: Flavor,
    pub changed: usize,
    pub refusal: Option<Refusal>,
    pub property_patch: Option<usize>,
}

pub(crate) struct Frame {
    pub absolute: usize,
    pub length: usize,
    pub mask: usize,
    pub count_at: usize,
    pub data_end: usize,
    pub offsets: Vec<usize>,
    pub capacity: usize,
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn integer(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

pub(crate) fn frame(bytes: &[u8], row: usize) -> Frame {
    assert!(row < 3);
    let definition = DEFINITION_PAGE * PAGE;
    assert_eq!(word(bytes, definition + 45), 16);
    assert_eq!(integer(bytes, definition + 51), 3);
    let descriptor = definition + 63 + 3 * 12 + TEXT_NUMBER * 25;
    assert_eq!(bytes[descriptor], 10);
    assert_eq!(word(bytes, descriptor + 5), TEXT_NUMBER);
    assert_eq!(&bytes[descriptor + 15..descriptor + 17], &[2, 1]);
    assert_eq!(word(bytes, descriptor + 7), 0);
    assert_eq!(word(bytes, descriptor + 23), 510);
    let page = DATA_PAGE * PAGE;
    assert_eq!(word(bytes, page + 12), 3);
    let first = word(bytes, page + 14 + row * 2);
    let last = if row == 0 {
        PAGE
    } else {
        word(bytes, page + 12 + row * 2)
    };
    assert_eq!(first & 0xe000, 0);
    assert!(20 <= first && first < last && last <= PAGE);
    let absolute = page + first;
    let length = last - first;
    assert_eq!(word(bytes, absolute), 16);
    let mask = length - 2;
    let count_at = mask - 2;
    assert_eq!(word(bytes, absolute + count_at), 5);
    let data_end = count_at - 12;
    let offsets = (0..6)
        .map(|index| word(bytes, absolute + count_at - (index + 1) * 2))
        .collect::<Vec<_>>();
    assert_eq!(offsets[0], 130);
    assert!(offsets.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(offsets[5] <= data_end);
    let capacity = data_end - (offsets[5] - (offsets[1] - offsets[0]));
    Frame {
        absolute,
        length,
        mask,
        count_at,
        data_end,
        offsets,
        capacity,
    }
}

pub(crate) fn read_text(bytes: &[u8], row: usize) -> Option<String> {
    let frame = frame(bytes, row);
    if bytes[frame.absolute + frame.mask] & 2 == 0 {
        return None;
    }
    let payload = &bytes[frame.absolute + frame.offsets[0]..frame.absolute + frame.offsets[1]];
    if payload.starts_with(&[0xff, 0xfe]) {
        assert!(payload[2..].iter().all(|byte| *byte < 128 && *byte != 0));
        return Some(payload[2..].iter().map(|byte| char::from(*byte)).collect());
    }
    let (units, remainder) = payload.as_chunks::<2>();
    assert!(remainder.is_empty());
    Some(
        String::from_utf16(
            &units
                .iter()
                .map(|unit| u16::from_le_bytes(*unit))
                .collect::<Vec<_>>(),
        )
        .unwrap(),
    )
}

fn text(template: &text_model::Case) -> Text {
    Text {
        row: template.framing.row,
        expected: template.original,
        replacement: template.replacement,
        payload: template.payload,
    }
}

fn fixed(column: Column, updates: &[(usize, Cell, Cell)]) -> Group {
    Group {
        table: "Items",
        name: column.name(),
        column,
        updates: updates
            .iter()
            .map(|&(row, expected, replacement)| fixed_model::Assignment {
                row,
                expected,
                replacement,
            })
            .collect(),
    }
}

fn compose_text(before: &[u8], after: &mut [u8], update: &Text) {
    assert_eq!(
        read_text(before, update.row),
        update.expected.map(str::to_owned)
    );
    if update.expected == update.replacement {
        return;
    }
    let layout = frame(before, update.row);
    let payload = update.payload.unwrap();
    assert!(payload.len() <= layout.capacity);
    let original = &before[layout.absolute..layout.absolute + layout.length];
    let record = &mut after[layout.absolute..layout.absolute + layout.length];
    let new_stop = layout.offsets[0] + payload.len();
    let final_end = new_stop + layout.offsets[5] - layout.offsets[1];
    assert!(final_end <= layout.data_end);
    record[layout.offsets[0]..new_stop].copy_from_slice(payload);
    record[new_stop..final_end].copy_from_slice(&original[layout.offsets[1]..layout.offsets[5]]);
    for (index, offset) in layout.offsets.iter().enumerate() {
        let value = if index == 0 {
            *offset
        } else {
            new_stop + offset - layout.offsets[1]
        };
        let at = layout.count_at - (index + 1) * 2;
        record[at..at + 2].copy_from_slice(&u16::try_from(value).unwrap().to_le_bytes());
    }
    record[layout.mask] =
        (record[layout.mask] & !2) | if update.replacement.is_some() { 2 } else { 0 };
}

fn payload(cell: Cell) -> Vec<u8> {
    match cell.value() {
        Value::Int32(value) => value.to_le_bytes().to_vec(),
        Value::Int64(value) => value.to_le_bytes().to_vec(),
        Value::Currency(value) => value.scaled().to_le_bytes().to_vec(),
        Value::DateTime(value) => value.to_bits().to_le_bytes().to_vec(),
        Value::Null | Value::Boolean(_) => Vec::new(),
        _ => unreachable!("closed independently authored fixed family"),
    }
}

fn apply_fixed(before: &[u8], after: &mut [u8], group: &Group, update: &fixed_model::Assignment) {
    assert_eq!(
        fixed_model::read(before, group.column, update.row),
        update.expected
    );
    if update.expected == update.replacement {
        return;
    }
    let (at, mask) = fixed_model::location(before, group.column, update.row);
    let bit = 1 << (group.column.number() % 8);
    match update.replacement {
        Cell::Boolean(value) => {
            assert!(matches!(group.column, Column::Yes | Column::No));
            if value {
                after[mask] |= bit;
            } else {
                after[mask] &= !bit;
            }
        }
        Cell::Null => after[mask] &= !bit,
        value => {
            let encoded = payload(value);
            assert_eq!(encoded.len(), group.column.width());
            after[at..at + encoded.len()].copy_from_slice(&encoded);
            after[mask] |= bit;
        }
    }
}

pub(crate) fn reconstruct(
    before: &[u8],
    texts: &[Text],
    fixed_groups: &[Group],
) -> (Vec<u8>, usize) {
    let mut after = before.to_vec();
    let mut rows = BTreeSet::new();
    let mut changed = BTreeSet::new();
    for update in texts {
        assert!(rows.insert(update.row));
        compose_text(before, &mut after, update);
        if update.expected != update.replacement {
            changed.insert(update.row);
        }
    }
    let mut columns = BTreeSet::new();
    for group in fixed_groups {
        assert_eq!(group.table, "Items");
        assert_eq!(group.name, group.column.name());
        assert!(columns.insert(group.column.number()));
        let mut ordinals = BTreeSet::new();
        for update in &group.updates {
            assert!(ordinals.insert(update.row));
            apply_fixed(before, &mut after, group, update);
            if update.expected != update.replacement {
                changed.insert(update.row);
            }
        }
    }
    for update in texts {
        assert_eq!(
            read_text(&after, update.row),
            update.replacement.map(str::to_owned)
        );
    }
    (after, changed.len())
}

fn original_properties(before: &[u8]) -> &[u8] {
    let locations = before
        .windows(4)
        .enumerate()
        .filter_map(|(at, value)| (value == b"MR2\0").then_some(at))
        .collect::<Vec<_>>();
    assert_eq!(locations.len(), 1);
    let start = locations[0];
    assert!(start >= 12);
    let envelope = integer(before, start - 12);
    assert_eq!(envelope & 0xc000_0000, 0x8000_0000);
    assert_eq!(&before[start - 8..start], &[0; 8]);
    let end = start + (envelope & 0x3fff_ffff);
    assert!(end <= before.len());
    &before[start..end]
}

fn require_text(before: &[u8]) -> (Vec<u8>, usize) {
    // The existing selector proves the exact Short_Text group and ALZ entry.
    // This authored owner places its 9-byte Required entry immediately before
    // ALZ. Validate both literal entries rather than adding a general parser.
    let (_, alz) = zero_length_source::disallow_empty(before, original_properties(before));
    let required = alz.checked_sub(9).unwrap();
    assert_eq!(
        &before[required - 8..required + 1],
        &[9, 0, 1, 1, 10, 0, 1, 0, 0]
    );
    assert_eq!(&before[alz - 8..alz + 1], &[9, 0, 1, 1, 11, 0, 1, 0, 1]);
    let mut result = before.to_vec();
    result[required] = 1;
    assert_eq!(&result[..required], &before[..required]);
    assert_eq!(&result[required + 1..], &before[required + 1..]);
    (result, required)
}

pub(crate) fn source(code: u8, recipe: Recipe) -> Case {
    let text_recipe = match recipe {
        Recipe::UnicodeCurrency => text_model::Recipe::ShorterUnicode,
        Recipe::SameCapacityDate => text_model::Recipe::SameCapacityUnicode,
        Recipe::RawTextNoopFixed | Recipe::Int32TextNoopPadding | Recipe::AllNoops => {
            text_model::Recipe::Noop
        }
        Recipe::NullTextNoopFixed | Recipe::BothInactiveNoopsWithBoolean => {
            text_model::Recipe::NullNoop
        }
        Recipe::TextToNullMasks | Recipe::TextRequired => text_model::Recipe::ValueToNull,
        Recipe::NullTextToEmptyMasks => text_model::Recipe::NullToValue,
        Recipe::OverflowAfterFixed => text_model::Recipe::Overflow,
        Recipe::DisallowEmptyText => text_model::Recipe::Empty,
        _ => text_model::Recipe::ShorterNarrow,
    };
    let template = text_model::source(code, text_recipe);
    assert_eq!(template.admitted_source, code == 6);
    let mut texts = vec![text(&template)];
    let mut before = template.before;
    let mut flavor = Flavor::Rich;
    let mut refusal = None;
    let mut property_patch = None;
    let fixed_groups = match recipe {
        Recipe::BetaLarge => vec![fixed(Large, &[(0, L(1000), L(-101))])],
        Recipe::UnicodeCurrency => vec![fixed(Money, &[(0, C(12345), C(-10001))])],
        Recipe::SameCapacityDate => vec![fixed(Date, &[(0, D(0), D(0x8000_0000_0000_0000))])],
        Recipe::BetaBoolean => vec![fixed(Yes, &[(0, B(false), B(true))])],
        Recipe::RawTextNoopFixed => vec![
            fixed(Large, &[(0, L(1000), L(-101))]),
            fixed(Money, &[(0, C(12345), N)]),
        ],
        Recipe::NullTextNoopFixed => vec![
            fixed(Money, &[(1, C(24690), C(-10001))]),
            fixed(Yes, &[(1, B(false), B(true))]),
        ],
        Recipe::TextToNullMasks => vec![
            fixed(Large, &[(0, L(1000), N)]),
            fixed(Date, &[(0, D(0), N)]),
            fixed(Yes, &[(0, B(false), B(true))]),
        ],
        Recipe::NullTextToEmptyMasks => {
            texts[0].replacement = Some("");
            texts[0].payload = Some(EMPTY);
            vec![
                fixed(Large, &[(1, L(2000), N)]),
                fixed(Date, &[(1, D(0), N)]),
                fixed(Yes, &[(1, B(false), B(true))]),
            ]
        }
        Recipe::TextChangeInactiveDateNoop | Recipe::BothInactiveNoopsWithBoolean => {
            let row = if recipe == Recipe::TextChangeInactiveDateNoop {
                0
            } else {
                1
            };
            let (payload, mask) = fixed_model::location(&before, Date, row);
            before[mask] &= !(1 << 5);
            before[payload..payload + 8].fill(255);
            if row == 0 {
                vec![
                    fixed(Date, &[(0, N, N)]),
                    fixed(Large, &[(0, L(1000), L(-101))]),
                ]
            } else {
                vec![
                    fixed(Date, &[(1, N, N)]),
                    fixed(Yes, &[(1, B(false), B(true))]),
                ]
            }
        }
        Recipe::DisjointRows => {
            let second = text_model::source(code, text_model::Recipe::NullToValue);
            assert_eq!(before, second.before);
            texts.push(text(&second));
            vec![
                fixed(Large, &[(1, L(2000), L(-101)), (2, L(3000), N)]),
                fixed(Yes, &[(2, B(false), B(true))]),
            ]
        }
        Recipe::Int32TextChange => {
            flavor = Flavor::Int32;
            before = fixed_model::source(code, fixed_model::Recipe::Int32Boolean).before;
            vec![
                fixed(LargeInt32, &[(0, I(1000), I(101))]),
                fixed(Yes, &[(0, B(false), B(true))]),
            ]
        }
        Recipe::Int32TextNoopPadding | Recipe::Int32NullPaddingText => {
            flavor = Flavor::Int32;
            before = fixed_model::source(code, fixed_model::Recipe::Int32NullPadding).before;
            let assignments = if recipe == Recipe::Int32NullPaddingText {
                vec![(0, N, I(101)), (1, I(2000), N)]
            } else {
                vec![(0, N, I(101))]
            };
            vec![
                fixed(LargeInt32, &assignments),
                fixed(Money, &[(1, C(24690), C(-10001))]),
                fixed(Yes, &[(1, B(false), B(true))]),
            ]
        }
        Recipe::AllNoops => vec![
            fixed(Large, &[(0, L(1000), L(1000))]),
            fixed(Money, &[(0, C(12345), C(12345))]),
            fixed(Date, &[(0, D(0), D(0))]),
            fixed(Yes, &[(0, B(false), B(false))]),
        ],
        Recipe::EmptyTextFixedChange => {
            texts.clear();
            vec![
                fixed(Large, &[(0, L(1000), L(-101))]),
                fixed(Yes, &[(0, B(false), B(true))]),
            ]
        }
        Recipe::FixedEmptyTextChange => vec![fixed(Large, &[]), fixed(Date, &[]), fixed(Yes, &[])],
        Recipe::EntirelyEmpty => {
            texts.clear();
            vec![
                fixed(Large, &[]),
                fixed(Date, &[]),
                fixed(Money, &[]),
                fixed(Yes, &[]),
            ]
        }
        Recipe::OverflowAfterFixed => {
            refusal = Some(Refusal::RowLayout);
            assert!(texts[0].payload.is_none());
            vec![fixed(Large, &[(0, L(1000), L(-101))])]
        }
        Recipe::DisallowEmptyText => {
            let (derivative, patch) =
                zero_length_source::disallow_empty(&before, original_properties(&before));
            before = derivative;
            property_patch = Some(patch);
            refusal = Some(Refusal::EmptyText);
            vec![fixed(Large, &[(0, L(1000), L(-101))])]
        }
        Recipe::TextRequired => {
            let promotion = text_model::source(code, text_model::Recipe::NullToValue);
            let original = before.clone();
            compose_text(&original, &mut before, &text(&promotion));
            assert_eq!(read_text(&before, 1).as_deref(), Some("é"));
            let (derivative, patch) = require_text(&before);
            before = derivative;
            property_patch = Some(patch);
            refusal = Some(Refusal::Required);
            vec![fixed(Yes, &[(0, B(false), B(true))])]
        }
        Recipe::NumericRequired => {
            let original = fixed_model::source(code, fixed_model::Recipe::LateRequired);
            before = original.before;
            property_patch = original.required_patch;
            refusal = Some(Refusal::Required);
            vec![fixed(Money, &[(0, C(12345), N)])]
        }
        Recipe::LateStaleText => {
            texts[0].expected = Some("ALPHA");
            refusal = Some(Refusal::Conflict);
            vec![fixed(Large, &[(0, L(1000), L(-101))])]
        }
        Recipe::LateStaleFixed => {
            refusal = Some(Refusal::Conflict);
            vec![fixed(Large, &[(0, L(999), L(-101))])]
        }
        Recipe::WrongFixedKind => {
            refusal = Some(Refusal::Column);
            vec![fixed(Money, &[(0, L(12345), L(-10001))])]
        }
        Recipe::EmptyIndexedFixed => {
            refusal = Some(Refusal::IndexedColumn);
            vec![fixed(Column::IndexedNumber, &[])]
        }
        Recipe::DuplicateTextRow => {
            texts.push(texts[0]);
            refusal = Some(Refusal::Conflict);
            vec![fixed(Yes, &[(0, B(false), B(true))])]
        }
    };
    assert!(!fixed_groups.is_empty());
    let (expected, changed) = if refusal.is_none() {
        let (expected, changed) = reconstruct(&before, &texts, &fixed_groups);
        (Some(expected), changed)
    } else {
        (None, 0)
    };
    let literal_count = match recipe {
        Recipe::DisjointRows => 3,
        Recipe::Int32TextNoopPadding | Recipe::Int32NullPaddingText => 2,
        Recipe::AllNoops
        | Recipe::EntirelyEmpty
        | Recipe::OverflowAfterFixed
        | Recipe::DisallowEmptyText
        | Recipe::TextRequired
        | Recipe::NumericRequired
        | Recipe::LateStaleText
        | Recipe::LateStaleFixed
        | Recipe::WrongFixedKind
        | Recipe::EmptyIndexedFixed
        | Recipe::DuplicateTextRow => 0,
        _ => 1,
    };
    assert_eq!(changed, literal_count);
    Case {
        before,
        expected,
        texts,
        fixed: fixed_groups,
        flavor,
        changed,
        refusal,
        property_patch,
    }
}

pub(crate) fn assert_protected(case: &Case) {
    if let Some(patch) = case.property_patch {
        assert_eq!(
            case.before[patch],
            u8::from(case.refusal == Some(Refusal::Required))
        );
    }
    let Some(expected) = &case.expected else {
        assert!(case.refusal.is_some());
        return;
    };
    assert_eq!(expected.len(), case.before.len());
    let touched = case
        .texts
        .iter()
        .map(|update| update.row)
        .chain(
            case.fixed
                .iter()
                .flat_map(|group| group.updates.iter().map(|update| update.row)),
        )
        .collect::<BTreeSet<_>>();
    let mut spans = Vec::new();
    for row in 0..3 {
        let old = frame(&case.before, row);
        let new = frame(expected, row);
        assert_eq!(
            (
                old.absolute,
                old.length,
                old.mask,
                old.count_at,
                old.data_end
            ),
            (
                new.absolute,
                new.length,
                new.mask,
                new.count_at,
                new.data_end
            )
        );
        if !touched.contains(&row) {
            assert_eq!(
                &case.before[old.absolute..old.absolute + old.length],
                &expected[new.absolute..new.absolute + new.length]
            );
            continue;
        }
        spans.push(old.absolute..old.absolute + old.length);
        let mut prefix_allowed = BTreeSet::new();
        let mut mask_allowed = [0_u8; 2];
        if case.texts.iter().any(|update| update.row == row) {
            mask_allowed[0] |= 2;
        }
        for group in &case.fixed {
            for update in group.updates.iter().filter(|update| update.row == row) {
                let (at, _) = fixed_model::location(&case.before, group.column, row);
                prefix_allowed.extend(at..at + group.column.width());
                mask_allowed[group.column.number() / 8] |= 1 << (group.column.number() % 8);
                if update.replacement == Cell::Null || update.expected == update.replacement {
                    assert_eq!(
                        &case.before[at..at + group.column.width()],
                        &expected[at..at + group.column.width()]
                    );
                }
            }
        }
        for at in old.absolute..old.absolute + old.offsets[0] {
            assert!(
                case.before[at] == expected[at] || prefix_allowed.contains(&at),
                "unselected prefix {at}"
            );
        }
        for variable in 1..5 {
            assert_eq!(
                &case.before[old.absolute + old.offsets[variable]
                    ..old.absolute + old.offsets[variable + 1]],
                &expected[new.absolute + new.offsets[variable]
                    ..new.absolute + new.offsets[variable + 1]],
                "opaque variable {variable}"
            );
        }
        assert_eq!(
            &case.before[old.absolute + new.offsets[5]..old.absolute + old.data_end],
            &expected[new.absolute + new.offsets[5]..new.absolute + new.data_end],
            "unused original padding"
        );
        for (index, allowed) in mask_allowed.iter().enumerate() {
            assert_eq!(
                (case.before[old.absolute + old.mask + index]
                    ^ expected[new.absolute + new.mask + index])
                    & !allowed,
                0
            );
        }
        if case.flavor == Flavor::Int32 {
            let (at, _) = fixed_model::location(&case.before, LargeInt32, row);
            assert_eq!(&case.before[at + 4..at + 8], &expected[at + 4..at + 8]);
        }
        if case
            .texts
            .iter()
            .any(|update| update.row == row && update.expected == update.replacement)
        {
            assert_eq!(old.offsets, new.offsets);
            assert_eq!(
                &case.before[old.absolute + old.offsets[0]..old.absolute + old.offsets[1]],
                &expected[new.absolute + new.offsets[0]..new.absolute + new.offsets[1]]
            );
        }
    }
    for (at, (before, after)) in case.before.iter().zip(expected).enumerate() {
        assert!(
            before == after || spans.iter().any(|span| span.contains(&at)),
            "outside record {at}"
        );
    }
    assert_eq!(
        reconstruct(&case.before, &case.texts, &case.fixed),
        (expected.clone(), case.changed)
    );
    let mut texts = case.texts.clone();
    texts.reverse();
    let mut fixed = case.fixed.clone();
    fixed.reverse();
    for group in &mut fixed {
        group.updates.reverse();
    }
    assert_eq!(
        reconstruct(&case.before, &texts, &fixed),
        (expected.clone(), case.changed)
    );
    if case.changed == 0 {
        assert_eq!(&case.before, expected);
    }
}
