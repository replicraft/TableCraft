//! Genuine map hosts preserve every existing referenced cell and physical census.
use super::{
    MAP_PAGE, PAGE, Source, canonical_blank_creation, extend_global_map, finish, native17,
};
use crate::fixtures::access::{packed_record::append_record, put32};
use tablecraft_core::values::LegacyDateTime;

#[derive(Clone, Copy, Debug)]
pub enum Hosts {
    Only,
    Alternate,
}

/// Extend the existing owner-zero host with valid reserved empty map rows.
/// Existing nonempty owned/candidate maps remain byte-exact and referenced.
/// An alternate host is genuinely owned through MSysRelationships' empty map.
pub fn before(existing_rows: usize, hosts: Hosts) -> Vec<u8> {
    let mut bytes = canonical_blank_creation::source();
    let count = usize::from(u16::from_le_bytes(
        bytes[MAP_PAGE * PAGE + 12..MAP_PAGE * PAGE + 14]
            .try_into()
            .unwrap(),
    ));
    assert!(existing_rows >= count);
    for _ in count..existing_rows {
        append_record(&mut bytes[MAP_PAGE * PAGE..(MAP_PAGE + 1) * PAGE], &[0; 13]);
    }

    if matches!(hosts, Hosts::Alternate) {
        let page = bytes.len() / PAGE;
        bytes.extend(native17::packed(0, vec![vec![0; 13]]));
        // MSysRelationships has zero DATA membership. Move its real empty owned
        // map reference to the added host; the old reserved empty row stays inert.
        assert_eq!(&bytes[5 * PAGE + 55..5 * PAGE + 59], &[1, 3, 0, 0]);
        put32(&mut bytes, 5 * PAGE + 55, (page as u32) << 8);
        extend_global_map(&mut bytes, page);
    }
    bytes
}

/// Independently author complete AFTER only for the chosen admissible host.
/// The first host stays full when an actual later referenced host is selected.
pub fn source(
    existing_rows: usize,
    hosts: Hosts,
    name: &str,
    alias: &str,
    value: i32,
    at: LegacyDateTime,
) -> Source {
    let before = before(existing_rows, hosts);
    let map_page = match hosts {
        Hosts::Only => MAP_PAGE,
        Hosts::Alternate => before.len() / PAGE - 1,
    };
    finish(before, map_page, name, alias, value, at)
}
