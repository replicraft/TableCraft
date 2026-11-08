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

/// Complete authored nine-page storage pair with full nullable catalog carriers
/// and represented, distinct DATA/LVAL map cells. Native system indexes are absent.
pub fn storage_pair() -> [Vec<u8>; 2] {
    use super::super::{
        canonical_creation as catalog, canonical_native17_creation as native17, put16, put32,
    };

    const PAGE: usize = 4096;
    const TABLES: u32 = 0x0f00_0001;
    let objects = [
        (0x0f00_0000, 0, "DatabaseRoot", 3, 0x8000_0002),
        (TABLES, 0x0f00_0000, "Tables", 3, 0x8000_0002),
        (0x0f00_0002, 0x0f00_0000, "Databases", 3, 0x8000_0002),
        (0x0f00_0003, 0x0f00_0000, "Relationships", 3, 0x8000_0002),
        (2, TABLES, "MSysObjects", 1, 0x8000_0002),
        (5, TABLES, "MSysRelationships", 1, 0x8000_0002),
        (6, TABLES, "MSysACEs", 1, 0x8000_0002),
        (7, TABLES, "MSysQueries", 1, 0x8000_0002),
        (QUERY_ID as u32, TABLES, "Saved", 5, 0),
    ];
    let catalog_rows = objects
        .into_iter()
        .map(|(id, parent, name, kind, flags)| {
            native17::catalog_row(&catalog::catalog_record(id, parent, name, kind, flags))
        })
        .collect::<Vec<_>>();
    let mut maps = vec![vec![0; 13]; 24];
    maps[0][5] = 0x10;
    maps[4][5] = 0x10;
    maps[3][6] = 0x01;
    maps[7][6] = 0x01;

    database_pair().map(|source| {
        let mut definition = source[14 * PAGE..15 * PAGE].to_vec();
        put32(&mut definition, 55, (3 << 8) | 3);
        put32(&mut definition, 59, (3 << 8) | 7);
        let mut tail = 63 + 8 * 25;
        for name in [
            "ObjectId",
            "Attribute",
            "Order",
            "Name1",
            "Name2",
            "Expression",
            "Flag",
            "LvExtra",
        ] {
            tail += 2 + name.encode_utf16().count() * 2;
        }
        put16(&mut definition, tail, 5);
        put32(&mut definition, tail + 2, (3 << 8) | 22);
        put32(&mut definition, tail + 6, (3 << 8) | 23);
        put16(&mut definition, tail + 10, 0xffff);
        put32(&mut definition, 8, (tail + 12) as u32);
        put16(&mut definition, 2, (PAGE - tail - 20) as u16);

        let mut data = source[16 * PAGE..17 * PAGE].to_vec();
        put32(&mut data, 4, 7);
        let low = usize::from(u16::from_le_bytes(data[22..24].try_into().unwrap()));
        put16(&mut data, 2, (low - 24) as u16);
        [
            catalog::header(),
            native17::packed(1, vec![vec![0; 13]]),
            native17::declaration(9),
            native17::packed(0, maps.clone()),
            native17::packed(2, catalog_rows.clone()),
            catalog::definition(&catalog::RELATIONSHIPS, 0, 1),
            catalog::definition(&catalog::ACES, 0, 2),
            definition,
            data,
        ]
        .concat()
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
