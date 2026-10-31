//! Original-request authority for NULL masks and inactive payload preservation.
use super::{budget, capture, nullable_sources::model};
use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate};
use tablecraft_core::values::Value;

fn requests(case: &model::Case) -> Vec<FixedScalarUpdate> {
    (0..case.requested_rows)
        .map(|row| {
            FixedScalarUpdate::new(
                row,
                case.initial[row].map_or(Value::Null, Value::Int32),
                case.replacement[row].map_or(Value::Null, Value::Int32),
            )
        })
        .collect()
}

#[test]
fn nullable_int32_proof_reconstructs_complete_original_mask_and_payload_intent() {
    for name in model::CASES {
        let case = model::source(name);
        let Some(expected) = &case.expected else {
            assert!(case.required);
            continue;
        };
        let updates = requests(&case);
        let groups = [FixedColumnUpdates::new("Items", "Large_Number", &updates)];
        let source = capture(&case.before);
        let changed = case
            .initial
            .iter()
            .zip(case.replacement)
            .filter(|(before, after)| **before != *after)
            .count();

        super::super::proof::prove(&source, &groups, expected, changed, &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(
            super::super::proof::prove(
                &source,
                &groups,
                expected,
                changed + 1,
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Preservation)
        );
    }
}

#[test]
fn nullable_int32_proof_rejects_inactive_payload_neighbor_mask_and_opaque_forgeries() {
    for name in ["null_noop", "value_to_null", "null_to_value", "mixed_rows"] {
        let case = model::source(name);
        let expected = case.expected.as_ref().unwrap();
        let updates = requests(&case);
        let groups = [FixedColumnUpdates::new("Items", "Large_Number", &updates)];
        let source = capture(&case.before);
        let changed = case
            .initial
            .iter()
            .zip(case.replacement)
            .filter(|(before, after)| **before != *after)
            .count();
        let (payload, mask, _) = model::row(&case.before, 0);

        for offset in [payload, mask, 31 * 4096] {
            let mut forged = expected.clone();
            // The target is bit 4. Flipping bit 0 corrupts its neighbor.
            forged[offset] ^= 1;
            assert_eq!(
                super::super::proof::prove(
                    &source,
                    &groups,
                    &forged,
                    changed,
                    &mut budget(),
                    &|| false,
                ),
                Err(EditError::Preservation),
                "Accepted {name} forgery at {offset}"
            );
        }
    }
}
