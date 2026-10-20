use super::*;
use crate::table::indexes::page::KeyRef;
use std::ops::Bound::{Excluded, Included, Unbounded};

fn key(bytes: &[u8]) -> Option<BinaryKey> {
    profile(false).decode(&encoded(bytes, false)).unwrap().0
}
fn reference(bytes: &[u8]) -> KeyRef<Option<BinaryKey>> {
    KeyRef {
        key: key(bytes),
        row: 0,
    }
}

#[test]
fn null_empty_open_closed_and_descending_hulls_are_explicit() {
    let empty = BinaryIndexSelection::Range {
        lower: Included(&[]),
        upper: Included(&[]),
    };
    assert!(empty.contains(key(&[])));
    assert!(!empty.contains(None));
    assert!(!empty.contains(key(&[0])));
    let reversed = BinaryIndexSelection::Range {
        lower: Included(&[2]),
        upper: Included(&[1]),
    };
    assert!(!reversed.may_match(None, None, false, 255));
    assert!(
        !BinaryIndexSelection::Range {
            lower: Excluded(&[1]),
            upper: Included(&[1])
        }
        .may_match(None, None, true, 255)
    );
    let exact = BinaryIndexSelection::Range {
        lower: Included(&[1]),
        upper: Included(&[1]),
    };
    assert!(!exact.may_match(Some(reference(&[2])), Some(reference(&[3])), false, 255));
    assert!(!exact.may_match(Some(reference(&[3])), Some(reference(&[2])), true, 255));
    let with_empty = BinaryIndexSelection::Range {
        lower: Unbounded,
        upper: Included(&[1]),
    };
    // Descending hull retains the gap between empty and smaller selected keys.
    assert!(with_empty.may_match(Some(reference(&[3])), Some(reference(&[2])), true, 255));
    assert!(BinaryIndexSelection::Null.contains(None));
    assert!(!BinaryIndexSelection::Null.contains(key(&[])));
    assert!(BinaryIndexSelection::All.validate().is_ok());
    let too_long = vec![0; MAX_SHORT_BINARY_BYTES + 1];
    assert!(matches!(
        BinaryIndexSelection::Range {
            lower: Included(&too_long),
            upper: Unbounded
        }
        .validate(),
        Err(InspectError::StructureLimit)
    ));
}

proptest! {
    #![proptest_config(proptest::test_runner::Config { cases:128,rng_seed:proptest::test_runner::RngSeed::Fixed(0x424e_4453),..Default::default() })]
    #[test]
    fn physical_hulls_never_prune_a_selected_candidate(a in proptest::collection::vec(any::<u8>(),0..16),b in proptest::collection::vec(any::<u8>(),0..16),closed_lower in any::<bool>(),closed_upper in any::<bool>(),descending in any::<bool>()) {
        let selected=BinaryIndexSelection::Range { lower:if closed_lower { Included(a.as_slice()) } else { Excluded(a.as_slice()) },upper:if closed_upper { Included(b.as_slice()) } else { Excluded(b.as_slice()) } };
        let mut values=vec![vec![],vec![0],vec![1],vec![255],vec![0,0],vec![1,0],a.clone(),b.clone()];
        values.sort_by_key(|value|encoded(value,descending));values.dedup();
        for lo in 0..values.len() {
            for hi in lo..values.len() {
                let expected=values[lo..=hi].iter().any(|value| (if closed_lower { value>=&a } else { value>&a }) && (if closed_upper { value<=&b } else { value<&b }));
                if expected { prop_assert!(selected.may_match(Some(reference(&values[lo])),Some(reference(&values[hi])),descending,MAX_SHORT_BINARY_BYTES)); }
            }
        }
    }
}
