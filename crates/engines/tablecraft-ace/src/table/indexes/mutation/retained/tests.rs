use super::{Preserved, RetainedProfile};
use crate::{
    InspectError,
    table::indexes::{
        binary::{BinaryIndexSelection, BinaryProfile},
        key::{KeyProfile, KeyWork},
    },
};
use tablecraft_core::limits::ResourceBudget;

#[test]
fn preserved_binary_keys_copy_original_segments_and_keep_semantic_order() {
    for descending in [false, true] {
        let profile = Preserved(BinaryProfile {
            column: 0,
            descending,
            maximum: 32,
            selection: BinaryIndexSelection::All,
        });
        let mut wires = vec![
            vec![0],
            vec![0x7f],
            vec![0x7f, 1, 0, 2, 0, 0, 0, 0, 0, 3],
            vec![0x7f, 255, 0, 0, 0, 0, 0, 0, 0, 1],
        ];
        if descending {
            for wire in &mut wires {
                wire[0] = !wire[0];
                for byte in &mut wire[1..] {
                    *byte = !*byte;
                }
            }
        }
        let mut budget = ResourceBudget::new(1_000_000, 100, 1_000_000);
        let mut work = KeyWork {
            budget: &mut budget,
            cancelled: &|| false,
        };
        let mut keys = Vec::new();
        for wire in &wires {
            let (key, consumed) = profile.decode(wire, &mut work).unwrap();
            assert_eq!(consumed, wire.len());
            let retained = profile.retain(&key, &mut work).unwrap();
            let mut output = [0xa7; 300];
            let length = profile.encode_retained(&retained, &mut output).unwrap();
            assert_eq!(&output[..length], wire);
            assert!(output[length..].iter().all(|byte| *byte == 0xa7));
            keys.push(key);
        }
        for (position, pair) in keys.windows(2).enumerate() {
            // Existing native Binary ordering keeps empty before nonempty in
            // either direction; NULL and nonempty values follow direction.
            let expected = if descending && position != 1 {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            };
            assert_eq!(profile.compare(&pair[0], &pair[1]), expected);
        }
    }
}

#[test]
fn retained_wire_allocations_and_cancellation_cannot_be_bypassed() {
    let profile = Preserved(BinaryProfile {
        column: 0,
        descending: false,
        maximum: 32,
        selection: BinaryIndexSelection::All,
    });
    let wire = [0x7f, 9, 0, 0, 0, 0, 0, 0, 0, 1];
    let mut budget = ResourceBudget::new(1_000, 100, 1_000);
    let key = profile
        .decode(
            &wire,
            &mut KeyWork {
                budget: &mut budget,
                cancelled: &|| false,
            },
        )
        .unwrap()
        .0;
    let mut exhausted = ResourceBudget::new(0, 100, 1_000);
    assert!(
        profile
            .retain(
                &key,
                &mut KeyWork {
                    budget: &mut exhausted,
                    cancelled: &|| false,
                }
            )
            .is_err()
    );
    assert!(matches!(
        profile.retain(
            &key,
            &mut KeyWork {
                budget: &mut budget,
                cancelled: &|| true,
            }
        ),
        Err(InspectError::Cancelled)
    ));
}
