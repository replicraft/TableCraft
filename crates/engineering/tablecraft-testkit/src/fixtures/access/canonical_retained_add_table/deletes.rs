//! Independent row DELETE images for appended generated Number/Text tables.
use super::word;
use crate::fixtures::access::raw_record_delete;

const PAGE: usize = 4096;

/// Remove actual logical roots without retiring storage or rewinding generation.
/// Native high-water counters, hidden records and inactive key tails survive.
pub fn after(before: &[u8], definition: u32, removed: &[usize]) -> Vec<u8> {
    let page = definition as usize * PAGE;
    assert_eq!(word(before, page + 41), 2);
    assert_eq!(word(before, page + 45), 2);
    assert_eq!(before[page + 75], 4);
    assert_eq!(before[page + 75 + 15], 7);
    assert!([4, 10].contains(&before[page + 100]));
    raw_record_delete::after(before, definition, removed)
}

/// Original terminal witness for a source-bound DELETE request, without codecs.
/// This follows actual same-owner forwarding rather than assuming one hop.
pub fn record(before: &[u8], definition: u32, root: usize) -> &[u8] {
    crate::fixtures::access::raw_forwarding_chain::record(before, definition + 1, root)
}
