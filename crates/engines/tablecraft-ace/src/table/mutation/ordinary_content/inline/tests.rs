//! Literal map records independently exercise window and polarity laws.
use super::InlineBitmap;
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn bitmap(bytes: &[u8]) -> InlineBitmap<'_> {
    InlineBitmap::capture(bytes, &mut budget(), &|| false).unwrap()
}

#[test]
fn inline_windows_borrow_variable_lengths_and_nonzero_base() {
    let bytes = [0, 61, 0, 0, 0, 0x81, 0x01];
    let map = bitmap(&bytes);
    assert!(!map.contains(60));
    assert_eq!(map.bit(60), None);
    assert_eq!(map.bit(62), Some(false));
    assert_eq!(map.bit(77), None);
    assert!(map.contains(61));
    assert!(map.contains(68));
    assert!(map.contains(69));
    assert!(!map.contains(70));
    assert_eq!(map.members(&mut budget(), &|| false).unwrap(), [61, 68, 69]);
    assert_eq!(map.bytes.as_ptr(), bytes[5..].as_ptr());
    assert!(!map.is_empty(&mut budget(), &|| false).unwrap());

    let empty = bitmap(&[0, 255, 255, 255, 255]);
    assert!(empty.is_empty(&mut budget(), &|| false).unwrap());
    assert!(empty.members(&mut budget(), &|| false).unwrap().is_empty());
}

#[test]
fn subset_compares_membership_across_different_window_spans() {
    let owned = bitmap(&[0, 61, 0, 0, 0, 0x81, 0x01]);
    let shifted = bitmap(&[0, 68, 0, 0, 0, 0x03]);
    let missing = bitmap(&[0, 68, 0, 0, 0, 0x07]);
    assert!(
        shifted
            .is_subset_of(owned, &mut budget(), &|| false)
            .unwrap()
    );
    assert!(
        !missing
            .is_subset_of(owned, &mut budget(), &|| false)
            .unwrap()
    );
    assert!(
        !owned
            .is_subset_of(shifted, &mut budget(), &|| false)
            .unwrap()
    );
}

#[test]
fn future_free_capacity_is_distinct_from_owned_and_candidate_pages() {
    // Native FREE metadata may represent capacity beyond EOF. Parsing the
    // record alone imposes no owned/candidate authority on those set bits.
    let free = bitmap(&[0, 64, 0, 0, 0, 0xff, 0x80]);
    assert!(free.contains(64));
    assert!(free.contains(79));
    // Ledger invokes this additional bound for every non-GLOBAL map identity.
    assert!(free.require_existing(64, &mut budget(), &|| false).is_err());
    free.require_existing(80, &mut budget(), &|| false).unwrap();

    let last = bitmap(&[0, 63, 0, 0, 0, 0x01]);
    last.require_existing(64, &mut budget(), &|| false).unwrap();
    assert!(last.require_existing(63, &mut budget(), &|| false).is_err());
}

#[test]
fn indirect_truncated_overflow_and_cancelled_maps_refuse() {
    for bytes in [
        &[1, 0, 0, 0, 0, 0][..],
        &[0, 0, 0, 0][..],
        &[0, 255, 255, 255, 255, 2][..],
    ] {
        assert!(InlineBitmap::capture(bytes, &mut budget(), &|| false).is_err());
    }
    let bytes = [0, 0, 0, 0, 0, 0];
    assert!(InlineBitmap::capture(&bytes, &mut budget(), &|| true).is_err());
    assert!(
        InlineBitmap::capture(
            &bytes,
            &mut ResourceBudget::new(1_000_000, 1_000_000, 0),
            &|| false,
        )
        .is_err()
    );
}
