//! Independent populated tail-field deletion images and literal survivor values.
pub mod readdition;
use super::super::{populated as number_rows, populated_mixed, populated_text};
use super::{Variant, remove_nonprimary};
use tablecraft_core::values::Value;

#[derive(Clone, Copy, Debug)]
pub enum Case {
    /// Original short rows and a later complete Number record retain their bytes.
    NumberPresent,
    /// The deleted Number is genuinely absent from every historical record.
    NumberAbsent,
    /// Repeated Text growth retains both the logical root and old hidden targets.
    TextPresent,
    /// The deleted Text has never had a physical variable slot in old records.
    TextAbsent,
    /// Removed original Text has present, empty and opaque NULL representations.
    MixedText,
    /// Raw binary siblings remain alongside compressed/wide/opaque NULL Text.
    MixedBinary,
    /// Removing the only property-bearing field keeps its unused dictionary.
    IdOnlyText,
}

pub const CASES: [Case; 7] = [
    Case::NumberPresent,
    Case::NumberAbsent,
    Case::TextPresent,
    Case::TextAbsent,
    Case::MixedText,
    Case::MixedBinary,
    Case::IdOnlyText,
];

pub fn column(case: Case) -> &'static str {
    match case {
        Case::NumberPresent | Case::NumberAbsent => "Bonus",
        Case::MixedText => "OtherLabel",
        _ => "NewLabel",
    }
}

/// Construct only through independent byte fixtures, never a native mutation API.
pub fn source(case: Case) -> Vec<u8> {
    match case {
        Case::NumberPresent => {
            let before = number_rows::pair("Bonus", &[Some(42), None, Some(-7)])[1].to_vec();
            number_rows::append_row(&before, Some(91), Some(37))
        }
        Case::NumberAbsent => number_rows::pair("Bonus", &[Some(42), None, Some(-7)])[1].to_vec(),
        Case::TextPresent => {
            let before = populated_text::pair("NewLabel", &[Some(42), None, Some(-7)])[1].to_vec();
            let first = populated_text::after(&before, 0, "first");
            populated_text::present::after(&first, 0, 2, Some("substantially larger retired text"))
        }
        Case::TextAbsent => {
            populated_text::pair("NewLabel", &[Some(42), None, Some(-7)])[1].to_vec()
        }
        Case::MixedText => populated_mixed::pair(populated_mixed::Case::Text)[0].to_vec(),
        Case::MixedBinary => {
            let before = populated_mixed::pair(populated_mixed::Case::Binary)[0].to_vec();
            let declared = populated_text::append_text(&before, "NewLabel");
            populated_text::after(&declared, 0, "retired")
        }
        Case::IdOnlyText => {
            let before = populated_text::id_only_pair()[1].to_vec();
            populated_text::after(&before, 0, "retired")
        }
    }
}

/// Remove the active declaration/name and precisely its live property group.
/// DATA, indexes, counters, native high-water marks and all old snapshots survive.
pub fn after(before: &[u8], selected: &str) -> Vec<u8> {
    remove_nonprimary(before, selected, Variant::AppendedGroupRemoved)
}

pub fn pair(case: Case) -> [Vec<u8>; 2] {
    let before = source(case);
    let after = after(&before, column(case));
    [before, after]
}

/// Literal survivor carriers are separate from every native reader and writer.
pub fn expected_rows(case: Case) -> Vec<Vec<Value>> {
    match case {
        Case::MixedText | Case::MixedBinary => {
            let mut rows = vec![
                vec![
                    Value::Int32(1),
                    Value::Text("alpha".into()),
                    Value::Int32(42),
                ],
                vec![Value::Int32(2), Value::Null, Value::Null],
                vec![
                    Value::Int32(3),
                    Value::Text("wide".into()),
                    Value::Int32(-7),
                ],
            ];
            if matches!(case, Case::MixedBinary) {
                rows[0].push(Value::Binary(vec![0, 255, 1, 2]));
                rows[1].push(Value::Binary(Vec::new()));
                rows[2].push(Value::Null);
            }
            rows
        }
        Case::IdOnlyText => vec![
            vec![Value::Int32(1)],
            vec![Value::Int32(2)],
            vec![Value::Int32(3)],
        ],
        _ => {
            let mut rows = vec![
                vec![Value::Int32(1), Value::Int32(42)],
                vec![Value::Int32(2), Value::Null],
                vec![Value::Int32(3), Value::Int32(-7)],
            ];
            if matches!(case, Case::NumberPresent) {
                rows.push(vec![Value::Int32(4), Value::Int32(91)]);
            }
            rows
        }
    }
}

/// Independently known dense SOURCE declaration identities; DROP retains them.
#[derive(Clone, Copy, Debug)]
pub struct Identity {
    pub native_number: u16,
    pub column_id: u16,
}

pub const fn identity(case: Case) -> Identity {
    let number = match case {
        Case::NumberPresent | Case::NumberAbsent | Case::TextPresent | Case::TextAbsent => 2,
        Case::MixedText => 3,
        Case::MixedBinary => 4,
        Case::IdOnlyText => 1,
    };
    Identity {
        native_number: number,
        column_id: number,
    }
}

/// Add one logically nonempty, well-framed opaque LvExtra stream. All other
/// catalog cells and slot identities are copied independently and remain exact.
pub fn with_table_extra(case: Case) -> Vec<u8> {
    use crate::fixtures::access::{data_page, put16};

    const PAGE: usize = 4096;
    const CATALOG: usize = 4 * PAGE;
    const DIRECTORY: usize = 14;
    const SLOT_OFFSET_MASK: u16 = 0x1fff;
    const MASK_BYTES: usize = 3;
    const VARIABLE_SLOTS: usize = 11;
    const TABLE_ID: u32 = 7;
    const INLINE: u32 = 0x8000_0000;

    fn word(bytes: &[u8], at: usize) -> usize {
        usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
    }

    let mut bytes = source(case);
    let page = &bytes[CATALOG..CATALOG + PAGE];
    let count = word(page, 12);
    let mut rows = Vec::new();
    let mut end = PAGE;
    let mut changed = 0;
    for slot in 0..count {
        let raw = u16::from_le_bytes(
            page[DIRECTORY + 2 * slot..DIRECTORY + 2 * slot + 2]
                .try_into()
                .unwrap(),
        );
        let start = usize::from(raw & SLOT_OFFSET_MASK);
        let old = &page[start..end];
        let row = if old.len() >= 6
            && word(old, 0) == 17
            && u32::from_le_bytes(old[2..6].try_into().unwrap()) == TABLE_ID
        {
            changed += 1;
            let mask = old.len() - MASK_BYTES;
            let count_at = mask - 2;
            assert_eq!(word(old, count_at), VARIABLE_SLOTS);
            assert_eq!(old[mask + 2] & 1, 0);
            let payload_end = word(old, count_at - 2 * (VARIABLE_SLOTS + 1));
            let extra_start = word(old, count_at - 2 * VARIABLE_SLOTS);
            assert_eq!(extra_start, payload_end);
            let mut payload = (INLINE | 1).to_le_bytes().to_vec();
            payload.extend([0; 8]);
            payload.push(0x7f);
            let mut current = old[..payload_end].to_vec();
            current.extend(&payload);
            current.extend_from_slice(&old[payload_end..]);
            put16(
                &mut current,
                count_at + payload.len() - 2 * (VARIABLE_SLOTS + 1),
                (payload_end + payload.len()) as u16,
            );
            current[mask + payload.len() + 2] |= 1;
            current
        } else {
            old.to_vec()
        };
        rows.push((raw & !SLOT_OFFSET_MASK, row));
        end = start;
    }
    assert_eq!(changed, 1);
    let mut rebuilt = data_page(2, &rows);
    let occupied: usize = rows.iter().map(|(_, row)| row.len()).sum();
    put16(
        &mut rebuilt,
        2,
        (PAGE - occupied - DIRECTORY - 2 * count) as u16,
    );
    bytes[CATALOG..CATALOG + PAGE].copy_from_slice(&rebuilt);
    bytes
}
