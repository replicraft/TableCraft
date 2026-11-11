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
    default_map(
        &["ANSI Query Mode"],
        entries
            .iter()
            .map(|&(flags, kind, value)| (0, flags, kind, value)),
    )
}

/// Explicit database Name AutoCorrect OFF settings; absence remains distinct.
/// Both entries are independent LONG/DDL zero values in the unnamed default map.
pub fn name_auto_correct_disabled() -> Vec<u8> {
    name_auto_correct_values(&[(1, 4, &0_i32.to_le_bytes()), (1, 4, &0_i32.to_le_bytes())])
}

/// Original normal-MR2 database values, in Track/Perform dictionary order.
/// This supports malformed/unknown settings without producing a decoder oracle.
/// LvProp uses narrow value framing; LvExtra's Wide-MR2 framing is distinct.
pub fn name_auto_correct_values(entries: &[(u8, u8, &[u8])]) -> Vec<u8> {
    assert!(entries.len() <= 2);
    default_map(
        &["Track Name AutoCorrect Info", "Perform Name AutoCorrect"],
        entries
            .iter()
            .enumerate()
            .map(|(index, &(flags, kind, value))| {
                (u16::try_from(index).unwrap(), flags, kind, value)
            }),
    )
}

fn default_map<'a>(
    names: &[&str],
    entries: impl IntoIterator<Item = (u16, u8, u8, &'a [u8])>,
) -> Vec<u8> {
    // Literal normal-MR2 empty map name: four-byte length plus two-byte name.
    let mut body = vec![6, 0, 0, 0, 0, 0];
    for (index, flags, kind, value) in entries {
        body.extend_from_slice(&u16::try_from(value.len() + 8).unwrap().to_le_bytes());
        body.extend_from_slice(&[flags, kind]);
        body.extend_from_slice(&index.to_le_bytes());
        body.extend_from_slice(&u16::try_from(value.len()).unwrap().to_le_bytes());
        body.extend_from_slice(value);
    }
    wide_properties::bag(&[
        wide_properties::names(names),
        wide_properties::block(0, &body),
    ])
}

/// Literal native17 MSysDb under the canonical Databases container.
/// The system role has no table definition; its inline LvProp remains inert data.
pub fn native17_database_record(properties: &[u8]) -> Vec<u8> {
    let mut row = vec![0; 32];
    put16(&mut row, 0, 17);
    put32(&mut row, 2, 0x0f00_0004);
    put32(&mut row, 6, 0x0f00_0002);
    put16(&mut row, 10, 2);
    put32(&mut row, 28, 0x8000_0000);
    row.extend("MSysDb".encode_utf16().flat_map(u16::to_le_bytes));
    let name_end = u16::try_from(row.len()).unwrap();
    row.extend((0x8000_0000 | u32::try_from(properties.len()).unwrap()).to_le_bytes());
    row.extend([0; 8]);
    row.extend(properties);
    let end = u16::try_from(row.len()).unwrap();
    row.extend(end.to_le_bytes());
    for slot in (0..11).rev() {
        let start = match slot {
            0 => 32,
            1..=8 => name_end,
            _ => end,
        };
        row.extend(start.to_le_bytes());
    }
    row.extend(11_u16.to_le_bytes());
    row.extend([0xbf, 0x40, 0]);
    row
}

/// Replace only the MSysDb role's inline properties in a literal native17 catalog.
/// Other catalog records retain their complete bytes; no production codec runs.
pub fn with_native17_properties(before: &[u8], properties: &[u8]) -> Vec<u8> {
    let page = &before[4 * PAGE_BYTES..5 * PAGE_BYTES];
    let count = usize::from(u16::from_le_bytes(page[12..14].try_into().unwrap()));
    let mut records = Vec::with_capacity(count);
    let mut end = PAGE_BYTES;
    let mut replaced = false;

    for slot in 0..count {
        let entry = 14 + 2 * slot;
        let start = usize::from(u16::from_le_bytes(
            page[entry..entry + 2].try_into().unwrap(),
        ));
        assert!(start < end, "unforwarded native17 catalog record");
        let row = &page[start..end];
        assert_eq!(&row[..2], &17_u16.to_le_bytes());
        if row[2..6] == 0x0f00_0004_u32.to_le_bytes() {
            assert!(!replaced, "unique MSysDb role");
            records.push(native17_database_record(properties));
            replaced = true;
        } else {
            records.push(row.to_vec());
        }
        end = start;
    }
    assert!(replaced, "existing MSysDb role");

    let mut after = before.to_vec();
    after[4 * PAGE_BYTES..5 * PAGE_BYTES]
        .copy_from_slice(&super::canonical_native17_creation::packed(2, records));
    after
}
