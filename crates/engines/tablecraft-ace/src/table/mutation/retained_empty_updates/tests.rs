//! The literal empty oracle is the entire independently authored original image.
use super::*;
use crate::{FixedScalarUpdate, SlotScalarUpdate};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{self as raw, Target, fields, joined_update},
    raw_forwarding_chain,
};

const SCOPE: CatalogScope = CatalogScope::new(3724);

fn budget() -> ResourceBudget {
    ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn score() -> [FittedColumnUpdates<'static>; 1] {
    [FittedColumnUpdates::Fixed {
        column: "Score",
        updates: &[],
    }]
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(actual.iter().zip(expected).position(|(a, b)| a != b), None);
}

#[test]
fn retained_empty_long_updates_preserve_every_byte_on_populated_empty_and_forwarded_sources() {
    let source = joined_update::source();
    let forwarded =
        raw_forwarding_chain::extend(&source.bytes, source.target_definition + 1, 0, 20);
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let empty = raw::append(&seed, "Empty", Target::GeneratedNumber);
    for (before, table) in [
        (&source.bytes, "Table1"),
        (&forwarded, "Table1"),
        (&empty, "Empty"),
    ] {
        let plan = image(before)
            .plan_retained_empty_row_updates(table, &score(), &mut budget(), &|| false)
            .unwrap();
        equal_image(plan.before_bytes(), before);
        equal_image(plan.after_bytes(), before);
        assert_eq!(plan.changed_rows(), 0);
    }
    assert!(matches!(
        image(&source.bytes)
            .plan_retained_row_updates("Table1", &score(), &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
}

#[test]
fn empty_requests_still_reject_generated_missing_duplicate_wrong_category_and_nonempty_targets() {
    let source = joined_update::source();
    let supplied = [FixedScalarUpdate::new(
        0,
        Value::Int32(17),
        Value::Int32(25),
    )];
    for groups in [
        vec![],
        vec![FittedColumnUpdates::Fixed {
            column: "ID",
            updates: &[],
        }],
        vec![FittedColumnUpdates::Fixed {
            column: "Missing",
            updates: &[],
        }],
        vec![
            FittedColumnUpdates::Fixed {
                column: "Score",
                updates: &[],
            },
            FittedColumnUpdates::Fixed {
                column: "score",
                updates: &[],
            },
        ],
        vec![FittedColumnUpdates::Variable {
            column: "Score",
            updates: &[] as &[SlotScalarUpdate],
        }],
        vec![FittedColumnUpdates::Fixed {
            column: "Score",
            updates: &supplied,
        }],
    ] {
        assert!(
            image(&source.bytes)
                .plan_retained_empty_row_updates("Table1", &groups, &mut budget(), &|| false)
                .is_err()
        );
    }
}

#[test]
fn zero_actions_keep_ownership_cancellation_resource_and_complete_after_proof_checks() {
    let source = joined_update::source();
    assert!(matches!(
        image(&source.bytes).plan_retained_empty_row_updates(
            "Table1",
            &score(),
            &mut budget(),
            &|| true
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert!(
        image(&source.bytes)
            .plan_retained_empty_row_updates(
                "Table1",
                &score(),
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
    let mut corrupt = source.bytes.to_vec();
    let data = (source.target_definition as usize + 1) * 4096;
    corrupt[data + 4..data + 8].copy_from_slice(&2_u32.to_le_bytes());
    assert!(
        image(&corrupt)
            .plan_retained_empty_row_updates("Table1", &score(), &mut budget(), &|| false)
            .is_err()
    );
    let native = image(&source.bytes);
    let mut forged = source.bytes.to_vec();
    let last = forged.len() - 1;
    forged[last] ^= 1;
    assert!(matches!(
        super::proof::prove(&native, "Table1", &score(), &forged, &mut budget(), &|| {
            false
        }),
        Err(EditError::Preservation)
    ));
}
