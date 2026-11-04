//! Literal database-property catalog roles and narrow MR2 query-mode values.
use super::{data_page, mutation_fixture, put16, put32, record, wide_properties};

const PAGE_BYTES: usize = 4096;

/// A local scalar table plus a real Databases/MSysDb catalog containment pair.
/// Property values are inert raw bytes; None stores an absent LvProp value.
pub fn database(properties: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = mutation_fixture::database();
    let mut database = record(21, 20, 2, 0x80000000, b"\xff\xfeMSysDb");
    if let Some(properties) = properties {
        database.truncate(database.len() - 7);
        put16(&mut database, 0, 6);
        let start = u16::try_from(database.len()).unwrap();
        database.extend_from_slice(&(0x80000000 | properties.len() as u32).to_le_bytes());
        database.extend_from_slice(&[0; 8]);
        database.extend_from_slice(properties);
        database.extend_from_slice(&u16::try_from(database.len()).unwrap().to_le_bytes());
        database.extend_from_slice(&start.to_le_bytes());
        database.extend_from_slice(&16_u16.to_le_bytes());
        database.extend_from_slice(&2_u16.to_le_bytes());
        database.push(0x3f);
    }
    let objects = [
        (0, record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects")),
        (0, record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships")),
        (0, record(10, 0, 1, 0, b"\xff\xfeItems")),
        (
            0,
            record(20, 0x0f000000, 3, 0x80000000, b"\xff\xfeDatabases"),
        ),
        (0, database),
    ];
    put32(&mut bytes, 2 * PAGE_BYTES + 16, objects.len() as u32);
    bytes[4 * PAGE_BYTES..5 * PAGE_BYTES].copy_from_slice(&data_page(2, &objects));
    bytes
}

/// Independently framed property entries; duplicates remain available to tests.
pub fn query_modes(entries: &[(u8, u8, &[u8])]) -> Vec<u8> {
    let mut body = vec![6, 0, 0, 0, 0, 0];
    for (flags, kind, value) in entries {
        body.extend_from_slice(&u16::try_from(value.len() + 8).unwrap().to_le_bytes());
        body.extend_from_slice(&[*flags, *kind]);
        body.extend_from_slice(&0_u16.to_le_bytes());
        body.extend_from_slice(&u16::try_from(value.len()).unwrap().to_le_bytes());
        body.extend_from_slice(value);
    }
    wide_properties::bag(&[
        wide_properties::names(&["ANSI Query Mode"]),
        wide_properties::block(0, &body),
    ])
}
