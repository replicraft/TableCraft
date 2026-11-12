//! Literal UNIQUE declaration and complete CREATE/write/DROP history.
#[cfg(test)]
mod tests;

use super::{
    Identity, Row, Source, Target, definition::Properties, dropping, finish_named_with_properties,
    inserting, updating,
};
use crate::fixtures::access::{
    canonical_retained_add_table::{fields, populated},
    packed_record::append_record,
    put32, raw_forwarding_chain,
};
use std::collections::BTreeSet;

const PAGE: usize = 4096;

/// Complete consecutive images; every successful consumer starts at the prior image.
pub struct Case {
    /// Real unique CREATE input/output, followed by the original three-row UPDATE.
    pub updated: updating::Case,
    /// Append 14 after the successful UPDATE.
    pub inserted: Vec<u8>,
    /// Two supplied NULL requests follow the supplied 14.
    pub nulls: Vec<u8>,
    /// A third NULL comes from a distinct original Omitted request.
    pub omitted: Vec<u8>,
    /// Remove the actual unique declaration and its exclusive physical footprint.
    pub dropped: Vec<u8>,
    /// The previously conflicting supplied 12 is now accepted without an index.
    pub duplicate_inserted: Vec<u8>,
    /// The previously conflicting original row 11 becomes another stored 12.
    pub duplicate_updated: Vec<u8>,
}

/// A logical alias retains the unique physical owner after the selected DROP.
pub struct AliasCase {
    pub creation: Source,
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
}

/// Both logical declarations own the same genuine unique tree and usage map.
pub fn alias(descending: bool) -> AliasCase {
    let creation = creation(descending, &[Some(1), Some(2), Some(3)]);
    let aliased = dropping::surviving_unique_alias(&creation, "UniqueAlias");
    AliasCase {
        creation,
        before: aliased.before,
        expected: aliased.expected,
    }
}

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

/// Emit UNIQUE at descriptor construction, with independently authored native 0x81.
/// Ordinary declaration writers continue to emit 0x80 through their closed entry.
pub fn creation(descending: bool, values: &[Option<i32>]) -> Source {
    let (before, definition) = source_rows(values);
    finish_creation(before, definition, descending, values)
}

/// Forward the first logical row through twenty real same-owner DATA links
/// before constructing the independent UNIQUE index image. No expected INDEX
/// page or production reader contributes the retained-row fixture bytes.
pub fn forwarded(descending: bool) -> Source {
    let values = [Some(1), Some(2), Some(3)];
    let (before, definition) = source_rows(&values);
    let before = raw_forwarding_chain::extend(&before, definition + 1, 0, 20);
    finish_creation(before, definition, descending, &values)
}

fn source_rows(values: &[Option<i32>]) -> (Vec<u8>, u32) {
    let mut nonnull = BTreeSet::new();
    assert!(values.iter().flatten().all(|value| nonnull.insert(*value)));
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / PAGE) as u32;
    (
        populated::numbers(&seed, "UniqueValues", values),
        definition,
    )
}

fn finish_creation(
    before: Vec<u8>,
    definition: u32,
    descending: bool,
    values: &[Option<i32>],
) -> Source {
    let rows = values
        .iter()
        .copied()
        .enumerate()
        .map(|(ordinal, score)| Row {
            score,
            reference: ((definition + 1 + (ordinal / 255) as u32) << 8) | (ordinal % 255) as u32,
        })
        .collect();
    finish_named_with_properties(
        before,
        definition,
        Target {
            table: "UniqueValues",
            column: "CopiedScore",
            native_number: 0,
            descending,
        },
        rows,
        Identity {
            index_name: "ValueUnique",
            logical: 0,
        },
        Properties::Unique,
    )
}

/// Genuine conflicting records before any index exists, for CREATE refusal tests.
pub fn duplicate_before() -> Vec<u8> {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    populated::numbers(&seed, "UniqueValues", &[Some(1), Some(1), Some(2)])
}

/// Exact two-row final-key swap, retaining the third row and original unique tree.
pub fn swapped(descending: bool) -> updating::Case {
    updating::finish(
        creation(descending, &[Some(1), Some(2), Some(3)]),
        0,
        vec![
            updating::Cell {
                row: 0,
                expected: Some(1),
                value: Some(2),
            },
            updating::Cell {
                row: 1,
                expected: Some(2),
                value: Some(1),
            },
        ],
    )
}

/// Actual CREATE → indexed UPDATE → manual INSERT/NULL/omission → unique DROP.
pub fn source(descending: bool) -> Case {
    let created = creation(descending, &[Some(1), Some(2), Some(3)]);
    let cells = created
        .rows
        .iter()
        .enumerate()
        .map(|(row, original)| updating::Cell {
            row,
            expected: original.score,
            value: original.score.map(|value| value + 10),
        })
        .collect();
    let updated = updating::finish(created, 0, cells);
    let inserted = inserting::after(&updated, &[Some(14)]);
    let nulls = inserting::after(&updated, &[Some(14), None, None]);
    let omitted = inserting::after(&updated, &[Some(14), None, None, None]);
    let dropped = dropping::from_unique_indexed(&updated.creation, &omitted);
    let duplicate_inserted =
        append_unindexed(&dropped, updated.creation.target_definition, Some(12));
    // NO_CLONE: Each chronological expected image remains immutable for its own
    // consuming write check while the next independent complete image is authored.
    let mut duplicate_updated = duplicate_inserted.to_vec();
    let reference = updated.creation.rows[0].reference;
    let range = raw_forwarding_chain::terminal_range(
        &duplicate_inserted,
        reference >> 8,
        (reference & 255) as usize,
    );
    let record = &mut duplicate_updated[range];
    assert_eq!(&record[..2], &[1, 0]);
    assert_eq!(&record[2..6], &11_i32.to_le_bytes());
    assert_eq!(record[6] & 1, 1);
    record[2..6].copy_from_slice(&12_i32.to_le_bytes());
    Case {
        updated,
        inserted,
        nulls,
        omitted,
        dropped,
        duplicate_inserted,
        duplicate_updated,
    }
}

/// A genuine current native-zero manual field after the unique declaration is gone.
/// This is not the separate native-one retired-field fixture or generated source.
fn append_unindexed(before: &[u8], definition: u32, value: Option<i32>) -> Vec<u8> {
    let base = definition as usize * PAGE;
    let data = (definition as usize + 1) * PAGE;
    assert_eq!(&before[base + 41..base + 47], &[1, 0, 0, 0, 1, 0]);
    assert_eq!(&before[base + 47..base + 55], &[0; 8]);
    assert_eq!(before[base + 63], 4);
    assert_eq!(&before[base + 68..base + 70], &[0, 0]);
    assert_eq!(&before[base + 78..base + 80], &[3, 0]);
    assert_eq!(&before[base + 84..base + 88], &[0, 0, 4, 0]);
    let count = word(before, data + 12);
    assert!(count < 255);
    let mut record = vec![1, 0];
    record.extend(value.unwrap_or(0).to_le_bytes());
    record.push(u8::from(value.is_some()));
    // NO_CLONE: This full independent AFTER must coexist with its preserved
    // dropped-index BEFORE for exact complement and source-binding checks.
    let mut result = before.to_vec();
    append_record(&mut result[data..data + PAGE], &record);
    put32(&mut result, base + 16, long(before, base + 16) + 1);
    assert_eq!(&result[base + 20..base + 24], &before[base + 20..base + 24]);
    assert_eq!(result.len(), before.len());
    result
}
