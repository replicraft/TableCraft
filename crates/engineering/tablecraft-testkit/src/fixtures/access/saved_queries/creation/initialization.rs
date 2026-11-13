//! Independent first-query storage and complete resize images.
use super::{QueryStorage, Source, after_at};
use crate::fixtures::access::{
    canonical_blank_creation, canonical_creation as canonical,
    canonical_creation_autonumber as generated, canonical_creation_properties as properties,
    canonical_database_options as options, canonical_native17_creation as native17,
    canonical_retained_add_table::extend_global_map, packed_record::append_record, put16, put32,
    saved_queries::writable::QUERY_FIELDS,
};
use tablecraft_core::values::LegacyDateTime;

const PAGE: usize = 4096;
const MAP_PAGE: usize = 3;
const CATALOG_PAGE: usize = 4;
const CATALOG_DEFINITION: usize = 2;

#[derive(Clone, Copy, Debug)]
pub enum Case {
    Blank,
    Ordinary,
    GeneratedPrimary,
    RequiredText,
}

/// Every source is authored from independent raw schema laws, without an engine.
pub fn source(case: Case, name: &str, alias: &str, value: i32, at: LegacyDateTime) -> Source {
    let before = match case {
        Case::Blank => canonical_blank_creation::source(),
        Case::Ordinary => native17::source(canonical::source(canonical::Case::One).bytes),
        Case::GeneratedPrimary => {
            options::explicit_off(&generated::source(generated::Case::HighLast).bytes)
        }
        Case::RequiredText => properties::source(properties::Case::RequiredText).bytes,
    };
    let definition = before.len() / PAGE;
    let data = definition + 1;
    let initialized = initialize(&before, definition, data, at);
    // All authored original identities are positive. The new system table uses
    // its positive TDEF identity; the first query's signed ID is independent.
    let native_query_id = i32::MIN + 1;
    let expected = after_at(
        &initialized,
        native_query_id,
        name,
        alias,
        value,
        at,
        QueryStorage { definition, data },
    );
    Source {
        before,
        expected,
        native_query_id,
    }
}

/// A real system catalog role/TDEF exists, but its DATA belongs to another table.
/// This is partial storage, never authority to initialize a replacement role.
pub fn partial_query_storage(at: LegacyDateTime) -> Vec<u8> {
    let before = canonical_blank_creation::source();
    let definition = before.len() / PAGE;
    let data = definition + 1;
    let mut partial = initialize(&before, definition, data, at);
    assert_eq!(
        u32::from_le_bytes(
            partial[data * PAGE + 4..data * PAGE + 8]
                .try_into()
                .unwrap()
        ),
        definition as u32,
    );
    put32(&mut partial, data * PAGE + 4, 2);
    partial
}

fn initialize(
    before: &[u8],
    definition_page: usize,
    data_page: usize,
    at: LegacyDateTime,
) -> Vec<u8> {
    assert_eq!(before.len(), definition_page * PAGE);
    // NO_CLONE: The complete raw AFTER is independently mutable; callers retain
    // immutable BEFORE bytes for whole-image preservation and recovery checks.
    let mut bytes = before.to_vec();
    let maps = usize::from(u16::from_le_bytes(
        bytes[MAP_PAGE * PAGE + 12..MAP_PAGE * PAGE + 14]
            .try_into()
            .unwrap(),
    ));
    assert!(maps + 4 <= 256);
    for ordinal in 0..4 {
        let mut map = [0_u8; 13];
        if ordinal < 2 {
            let base = data_page / 64 * 64;
            map[1..5].copy_from_slice(&(base as u32).to_le_bytes());
            map[5 + (data_page - base) / 8] = 1 << ((data_page - base) % 8);
        }
        append_record(&mut bytes[MAP_PAGE * PAGE..(MAP_PAGE + 1) * PAGE], &map);
    }

    let mut definition = canonical::definition(&QUERY_FIELDS, 0, maps as u8);
    put32(
        &mut definition,
        59,
        (MAP_PAGE as u32) << 8 | (maps + 1) as u32,
    );
    let length = u32::from_le_bytes(definition[8..12].try_into().unwrap()) as usize;
    let tail = length - 2;
    assert_eq!(&definition[tail..length], &[255, 255]);
    put16(&mut definition, tail, 5);
    put32(
        &mut definition,
        tail + 2,
        (MAP_PAGE as u32) << 8 | (maps + 2) as u32,
    );
    put32(
        &mut definition,
        tail + 6,
        (MAP_PAGE as u32) << 8 | (maps + 3) as u32,
    );
    put16(&mut definition, tail + 10, 0xffff);
    put32(&mut definition, 8, (length + 10) as u32);
    put16(&mut definition, 2, (PAGE - length - 18) as u16);
    bytes.extend(definition);
    bytes.extend(native17::packed(definition_page as u32, Vec::new()));

    let mut catalog = native17::catalog_row(&canonical::catalog_record(
        definition_page as u32,
        0x0f00_0001,
        "MSysQueries",
        1,
        0x8000_0002,
    ));
    let timestamp = at.to_bits().to_le_bytes();
    catalog[12..20].copy_from_slice(&timestamp);
    catalog[20..28].copy_from_slice(&timestamp);
    append_record(
        &mut bytes[CATALOG_PAGE * PAGE..(CATALOG_PAGE + 1) * PAGE],
        &catalog,
    );
    let count = CATALOG_DEFINITION * PAGE + 16;
    let old = u32::from_le_bytes(bytes[count..count + 4].try_into().unwrap());
    put32(&mut bytes, count, old + 1);
    extend_global_map(&mut bytes, definition_page);
    bytes
}
