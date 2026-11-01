//! Pure physical geometry controls; no Source/admission or writer is invoked.
use super::{frontier, range};
use crate::{EditError, InspectError, format::reader::row_range};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};

fn page(words: &[u16]) -> Vec<u8> {
    let mut bytes = vec![0; 4096];
    bytes[..2].copy_from_slice(&[1, 1]);
    bytes[12..14].copy_from_slice(&(words.len() as u16).to_le_bytes());
    for (slot, word) in words.iter().enumerate() {
        let at = 14 + slot * 2;
        bytes[at..at + 2].copy_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 1_000, 1_000_000)
}

#[test]
fn deleted_empty_slots_retain_the_same_bounded_body_frontier() {
    let bytes = page(&[4030, 0x8fbe, 0xcfbe]);
    assert_eq!(range(&bytes, 0).unwrap(), (4030, 4030, 4096));
    assert_eq!(range(&bytes, 1).unwrap(), (0x8fbe, 4030, 4030));
    assert_eq!(range(&bytes, 2).unwrap(), (0xcfbe, 4030, 4030));
    assert_eq!(frontier(&bytes, &mut budget(), &|| false).unwrap(), 4030);
    // General Reader still rejects these empty physical extents.
    assert_eq!(row_range(&bytes, 1), Err(InspectError::Malformed));
    assert_eq!(
        frontier(&page(&[]), &mut budget(), &|| false).unwrap(),
        4096
    );
}

#[test]
fn placement_slots_refuse_live_empty_unsupported_reversed_and_out_of_bounds_words() {
    for words in [
        vec![4030, 4030],
        vec![4030, 0x4fbe],
        vec![4030, 0xafbe],
        vec![4030, 0x8fff],
        vec![4030, 0x8001],
        vec![0x9fff],
    ] {
        let bytes = page(&words);
        assert!(frontier(&bytes, &mut budget(), &|| false).is_err());
    }
    let bytes = page(&[4030, 0xafbe]);
    assert_eq!(range(&bytes, 1), Err(InspectError::Unsupported));
    assert_eq!(range(&bytes, 2), Err(InspectError::Malformed));
}

#[test]
fn placement_frontier_charges_work_and_checks_each_cancellation_boundary() {
    let bytes = page(&[4030, 0x8fbe, 0xcfbe]);
    let mut measured = budget();
    let steps = measured.remaining(Resource::Steps);
    let polls = Cell::new(0_usize);
    assert_eq!(
        frontier(&bytes, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap(),
        4030
    );
    let cost = steps - measured.remaining(Resource::Steps);
    assert!(cost > 3);
    assert_eq!(
        frontier(&bytes, &mut ResourceBudget::new(0, 0, cost), &|| false).unwrap(),
        4030
    );
    assert!(matches!(
        frontier(&bytes, &mut ResourceBudget::new(0, 0, cost - 1), &|| false),
        Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == Resource::Steps
    ));
    let terminal = polls.get();
    for stop in 1..=terminal {
        polls.set(0);
        assert!(matches!(
            frontier(&bytes, &mut budget(), &|| {
                polls.set(polls.get() + 1);
                polls.get() == stop
            }),
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
        assert_eq!(polls.get(), stop);
    }
}
