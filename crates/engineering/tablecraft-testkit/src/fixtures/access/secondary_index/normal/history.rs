//! Literal nondense logical-index history, followed by an unadorned new LONG.
use super::{Identity, Row, Source, Target, dropping, finish_named, protected_companion};
use crate::fixtures::access::{
    canonical_retained_add_table::{typed_fields::Kind, unadorned_fields},
    raw_forwarding_chain,
};

/// Shared-physical alias survives DROP, then OtherScore is added without policies.
/// Header count2 is occupied by surviving logical identity2; new identity3 is
/// authored explicitly instead of silently reusing the declaration count.
pub fn source() -> Source {
    source_with_direction(false)
}

/// Author the same identity history with an explicitly chosen new-index direction.
pub fn source_with_direction(descending: bool) -> Source {
    let original = protected_companion(false);
    let alias = dropping::surviving_alias(&original, "ScoreAlias");
    let before = unadorned_fields::after(
        &alias.expected,
        original.target_definition,
        "OtherScore",
        Kind::Int32,
        [0; 4],
    );
    let base = original.target_definition as usize * 4096;
    assert_eq!(&before[base + 41..base + 47], &[5, 0, 1, 0, 5, 0]);
    assert_eq!(&before[base + 47..base + 55], &[2, 0, 0, 0, 2, 0, 0, 0]);
    let page = &before[base..base + 4096];
    let mut physical = 87 + 5 * 25;
    for _ in 0..5 {
        physical += 2 + usize::from(u16::from_le_bytes(
            page[physical..physical + 2].try_into().unwrap(),
        ));
    }
    let logical = physical + 2 * 52;
    assert_eq!(
        u32::from_le_bytes(page[logical + 4..logical + 8].try_into().unwrap()),
        0
    );
    assert_eq!(
        u32::from_le_bytes(page[logical + 32..logical + 36].try_into().unwrap()),
        2
    );
    for row in &original.rows {
        assert_eq!(row.reference >> 8, original.target_definition + 1);
        let record = raw_forwarding_chain::record(
            &before,
            original.target_definition + 1,
            (row.reference & 255) as usize,
        );
        assert_eq!(
            &record[..2],
            &[4, 0],
            "new field4 is absent despite old variable payload at its future fixed offset"
        );
    }
    let rows = original
        .rows
        .iter()
        .map(|row| Row {
            score: None,
            reference: row.reference,
        })
        .collect();
    finish_named(
        before,
        original.target_definition,
        Target {
            table: original.table,
            column: "OtherScore",
            native_number: 4,
            descending,
        },
        rows,
        Identity {
            index_name: "OtherLookup",
            logical: 3,
        },
    )
}

#[cfg(test)]
mod tests;
