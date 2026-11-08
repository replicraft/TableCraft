//! Literal SELECT expression replacement oracles, without a mutation producer.
//!
//! The independent Jackcess observation in
//! `docs/testing/NATIVE_QUERYDEF_FITTING_SAVE.md` retains the same inline Memo
//! shape. These synthetic images omit the reference database's system indexes;
//! they establish byte expectations, not imported-source write eligibility.

use super::{Fragment, QUERY_ID, database_with_query_flags};

/// Original `SELECT 42 AS Answer` and the exact requested `43` replacement.
/// Query identity, fragment membership, ordering and all other bytes are retained.
pub fn database_pair() -> [Vec<u8>; 2] {
    ["42", "43"].map(|expression| {
        database_with_query_flags(
            &[
                Fragment {
                    object: Some(QUERY_ID),
                    attribute: Some(0),
                    order: Some(&[0, 0, 0, 1]),
                    ..Default::default()
                },
                Fragment {
                    object: Some(QUERY_ID),
                    attribute: Some(1),
                    flag: Some(1),
                    order: Some(&[0, 0, 0, 1]),
                    ..Default::default()
                },
                Fragment {
                    object: Some(QUERY_ID),
                    attribute: Some(3),
                    flag: Some(0),
                    order: Some(&[0, 0, 0, 1]),
                    ..Default::default()
                },
                Fragment {
                    object: Some(QUERY_ID),
                    attribute: Some(6),
                    expression: Some(expression),
                    name1: Some("Answer"),
                    order: Some(&[0, 0, 0, 1]),
                    ..Default::default()
                },
                Fragment {
                    object: Some(QUERY_ID),
                    attribute: Some(255),
                    order: Some(&[0, 0, 0, 1]),
                    ..Default::default()
                },
            ],
            0,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::database_pair;

    #[test]
    fn independent_projection_record_and_single_payload_byte_are_preserved() {
        let [before, after] = database_pair();
        // Independent observed record framing, using this raw fixture's literal
        // native ObjectId. No production field constants or codecs are used.
        let expected = [
            0x08, 0x00, 0x7b, 0x00, 0x00, 0x80, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x01, 0x41, 0x00, 0x6e, 0x00, 0x73, 0x00, 0x77, 0x00, 0x65, 0x00, 0x72,
            0x00, 0x04, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x34,
            0x00, 0x32, 0x00, 0x2d, 0x00, 0x1d, 0x00, 0x1d, 0x00, 0x11, 0x00, 0x0d, 0x00, 0x04,
            0x00, 0x2f,
        ];
        let matches = before
            .windows(expected.len())
            .enumerate()
            .filter_map(|(position, record)| (record == expected).then_some(position))
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        assert_eq!(before.len(), after.len());

        let changed = before
            .iter()
            .zip(&after)
            .enumerate()
            .filter_map(|(position, (old, new))| (old != new).then_some(position))
            .collect::<Vec<_>>();
        assert_eq!(changed, vec![matches[0] + 43]);
        assert_eq!(before[changed[0]], b'2');
        assert_eq!(after[changed[0]], b'3');
    }
}
