//! Full QueryDef append images with unrelated live table/index/property storage.
use super::{QueryStorage, Source, after_at};
use crate::fixtures::access::{
    canonical_creation_autonumber as generated, canonical_creation_properties as properties,
    canonical_database_options as options, put32,
    saved_queries::{EMPTY_ID, writable},
};
use tablecraft_core::values::LegacyDateTime;

#[derive(Clone, Copy, Debug)]
pub enum Case {
    Ordinary,
    GeneratedPrimary,
    RequiredText,
}

pub fn source(case: Case, name: &str, alias: &str, value: i32, at: LegacyDateTime) -> Source {
    let (before, definition, data) = match case {
        Case::Ordinary => (writable::source().before, 9, 10),
        Case::GeneratedPrimary => writable::append_to(options::explicit_off(
            &generated::source(generated::Case::HighLast).bytes,
        )),
        Case::RequiredText => {
            writable::append_to(properties::source(properties::Case::RequiredText).bytes)
        }
    };
    let native_query_id = EMPTY_ID + 1;
    let expected = after_at(
        &before,
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

/// The actual independent catalog row identifies MSysQueries' positive TDEF.
/// This raw owner reads its own literal native17 catalog framing, not engine metadata.
fn query_definition(before: &[u8]) -> usize {
    const PAGE: usize = 4096;
    let start = 4 * PAGE;
    let count = usize::from(u16::from_le_bytes(
        before[start + 12..start + 14].try_into().unwrap(),
    ));
    for slot in 0..count {
        let at = start + 14 + 2 * slot;
        let row = start
            + usize::from(u16::from_le_bytes(before[at..at + 2].try_into().unwrap()) & 0x1fff);
        let end = if slot == 0 {
            start + PAGE
        } else {
            let at = start + 12 + 2 * slot;
            start + usize::from(u16::from_le_bytes(before[at..at + 2].try_into().unwrap()) & 0x1fff)
        };
        let footer = end - 3 - 2 * 13;
        let first = usize::from(u16::from_le_bytes(
            before[footer + 22..footer + 24].try_into().unwrap(),
        ));
        let last = usize::from(u16::from_le_bytes(
            before[footer + 20..footer + 22].try_into().unwrap(),
        ));
        let name = "MSysQueries"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        if before.get(row + first..row + last) == Some(name.as_slice()) {
            return u32::from_le_bytes(before[row + 2..row + 6].try_into().unwrap()) as usize;
        }
    }
    panic!("the authored local-table source contains query storage")
}

pub fn after(
    before: &[u8],
    id: i32,
    name: &str,
    alias: &str,
    value: i32,
    at: LegacyDateTime,
) -> Vec<u8> {
    let definition = query_definition(before);
    after_at(
        before,
        id,
        name,
        alias,
        value,
        at,
        QueryStorage {
            definition,
            data: definition + 1,
        },
    )
}

/// Corrupt only the protected user DATA owner's native identity.
pub fn foreign_user_data_owner(mut before: Vec<u8>) -> Vec<u8> {
    put32(&mut before, 8 * 4096 + 4, 2);
    before
}

/// Set an unqualified nonzero flag on the literal ordinary user catalog row.
pub fn program_flag(mut before: Vec<u8>) -> Vec<u8> {
    let at = 4 * 4096 + 14 + 2 * 7;
    let row =
        4 * 4096 + usize::from(u16::from_le_bytes(before[at..at + 2].try_into().unwrap()) & 0x1fff);
    assert_eq!(
        i32::from_le_bytes(before[row + 2..row + 6].try_into().unwrap()),
        7
    );
    put32(&mut before, row + 28, 1);
    before
}

/// Change the genuine system database role's type while retaining its name,
/// parent, stored property body and system flags.
pub fn wrong_database_type(mut before: Vec<u8>) -> Vec<u8> {
    let row = database_row(&before);
    before[row + 10..row + 12].copy_from_slice(&5_i16.to_le_bytes());
    before
}

/// Remove the system designation from a real type-2 database role.
pub fn wrong_database_flags(mut before: Vec<u8>) -> Vec<u8> {
    let row = database_row(&before);
    put32(&mut before, row + 28, 0);
    before
}

fn database_row(before: &[u8]) -> usize {
    let at = 4 * 4096 + 14 + 2 * 8;
    let row =
        4 * 4096 + usize::from(u16::from_le_bytes(before[at..at + 2].try_into().unwrap()) & 0x1fff);
    assert_eq!(
        i16::from_le_bytes(before[row + 10..row + 12].try_into().unwrap()),
        2
    );
    assert_eq!(
        u32::from_le_bytes(before[row + 28..row + 32].try_into().unwrap()),
        0x8000_0002
    );
    row
}
