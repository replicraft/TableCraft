//! Test-only registry carrier using the actual literal MR2/payload builders.
use super::inspect::payload_fixture;
use tablecraft_testkit::access::{self as raw, wide_properties};

pub(crate) struct Carrier {
    pub record: Vec<u8>,
    pub extra: Vec<u8>,
}

pub(crate) fn carrier(code: u8, properties: &[(&str, &str)]) -> Carrier {
    let extra = wide_properties::event_properties(properties);
    let bytes =
        payload_fixture::database(code, &[None, None, Some(payload_fixture::inline(&extra))]);

    let page = &bytes[4 * 4096..5 * 4096];
    assert_eq!(u16::from_le_bytes(page[12..14].try_into().unwrap()), 3);
    let start = usize::from(u16::from_le_bytes(page[18..20].try_into().unwrap()));
    let end = usize::from(u16::from_le_bytes(page[16..18].try_into().unwrap()));
    assert_eq!(start & 0xe000, 0);
    assert!(start < end && end <= 4096);
    let mut record = page[start..end].to_vec();
    // Same original local-table catalog prefix correction as guard_sources.
    raw::put16(&mut record, 10, 1);

    Carrier { record, extra }
}
