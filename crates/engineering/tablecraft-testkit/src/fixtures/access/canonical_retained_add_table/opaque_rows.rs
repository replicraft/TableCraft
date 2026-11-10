//! Original metadata histories leave retired payloads opaque before a new LONG fill.
use super::{
    field_changes,
    fields::{self, Target},
};
use crate::fixtures::access::raw_forwarding_chain;
use tablecraft_core::values::Value;

const PAGE: usize = 4096;

#[derive(Clone, Copy, Debug)]
pub enum Case {
    FixedRetired,
    VariableRetired,
}

pub const CASES: [Case; 2] = [Case::FixedRetired, Case::VariableRetired];

pub struct Source {
    pub bytes: Vec<u8>,
    pub definition: u32,
    pub catalog_root: usize,
    pub roots: Vec<usize>,
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

pub fn source(case: Case) -> Source {
    let (mut initial, original) = match case {
        Case::FixedRetired => (fields::source(fields::Case::FirstNumber), "Score"),
        Case::VariableRetired => (fields::source(fields::Case::SecondText), "Name"),
    };
    if matches!(case, Case::VariableRetired) {
        let roots = std::mem::take(&mut initial.roots);
        let extended = fields::property_pages::extend_source(
            initial,
            fields::property_pages::Case::OriginalInsufficient,
        );
        initial = fields::Source {
            bytes: extended.bytes,
            definition: extended.definition,
            catalog_root: extended.catalog_root,
            roots,
        };
    }
    let number = fields::after(
        &initial.bytes,
        initial.definition,
        initial.catalog_root,
        Target::Number,
    );
    let text = fields::after(
        &number,
        initial.definition,
        initial.catalog_root,
        Target::ShortText,
    );
    let mut bytes = field_changes::rename(
        &text,
        initial.definition,
        initial.catalog_root,
        "NextText",
        "X",
    );
    for name in ["X", "NextNumber", original] {
        bytes = field_changes::drop_tail(&bytes, initial.definition, initial.catalog_root, name);
    }
    let bytes = fields::after(
        &bytes,
        initial.definition,
        initial.catalog_root,
        Target::Number,
    );
    let declaration =
        &bytes[initial.definition as usize * PAGE..(initial.definition as usize + 1) * PAGE];
    assert_eq!(word(declaration, 41), 5);
    assert_eq!(word(declaration, 45), 2);
    assert_eq!(&declaration[100..110], &[4, 89, 6, 0, 0, 4, 0, 0, 0, 1]);
    assert_eq!(word(declaration, 121), 4);
    for &root in &initial.roots {
        let record = raw_forwarding_chain::record(&bytes, initial.definition + 1, root);
        assert_eq!(word(record, 0), 2);
    }
    Source {
        bytes,
        definition: initial.definition,
        catalog_root: initial.catalog_root,
        roots: initial.roots,
    }
}

/// Observe the actual terminal record of this opaque LONG fixture family.
/// The selected declaration remains bound to logical field 4 and fixed offset 4.
pub fn record(before: &[u8], definition: u32, logical_root: usize) -> &[u8] {
    let declaration = &before[definition as usize * PAGE..(definition as usize + 1) * PAGE];
    assert_eq!(word(declaration, 45), 2);
    assert_eq!(&declaration[100..110], &[4, 89, 6, 0, 0, 4, 0, 0, 0, 1]);
    assert_eq!(word(declaration, 121), 4);
    raw_forwarding_chain::record(before, definition + 1, logical_root)
}

/// Compare actual logical presence, never ignored old bytes, before issuing an
/// independent complete-record AFTER. No-op requests retain the original image.
pub fn after(before: &[u8], definition: u32, logical_root: usize, value: Option<i32>) -> Vec<u8> {
    let declaration = &before[definition as usize * PAGE..(definition as usize + 1) * PAGE];
    assert_eq!(word(declaration, 45), 2);
    let selected = 100;
    assert_eq!(declaration[selected], 4);
    assert_eq!(word(declaration, selected + 5), 4);
    assert_eq!(word(declaration, selected + 21), 4);
    let record = raw_forwarding_chain::record(before, definition + 1, logical_root);
    let count = word(record, 0);
    let mask = record.len() - count.div_ceil(8);
    let present = count > 4 && record[mask] & 16 != 0;
    let original = if present {
        Some(i32::from_le_bytes(record[6..10].try_into().unwrap()))
    } else {
        None
    };
    if original == value {
        // NO_CLONE: The raw AFTER owns a separate mutable image so subsequent
        // fixture transformations cannot alter the caller's BEFORE baseline.
        return before.to_vec();
    }
    // No live variable field exists. The independent historical builder keeps
    // every byte before the original mask as a prefix, overwrites only this
    // declared LONG range, and appends a full retained-slot trailer and mask.
    fields::fill(
        before,
        definition,
        logical_root,
        Target::Number,
        value.map(Value::Int32),
    )
}

/// Append a fresh row without interpreting or rewriting opaque original tails.
/// The independent existing field-append oracle keeps its literal defaults,
/// retained footer count and physical-root allocation policy.
pub fn append(before: &[u8], definition: u32, input: super::rows::Input<'_>) -> Vec<u8> {
    let declaration = &before[definition as usize * PAGE..(definition as usize + 1) * PAGE];
    assert_eq!(word(declaration, 41), 5);
    assert_eq!(word(declaration, 45), 2);
    assert_eq!(&declaration[100..110], &[4, 89, 6, 0, 0, 4, 0, 0, 0, 1]);
    assert_eq!(word(declaration, 121), 4);
    fields::append(before, definition, Target::Number, input)
}
