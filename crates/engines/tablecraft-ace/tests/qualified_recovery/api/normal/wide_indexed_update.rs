//! Whole literal wider indexed UPDATE plans retain original TCNI4 requests.
use super::{Storage, budget, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{FixedScalarUpdate, NativeFixedIntentGroup, NativeImage, NativeUpdateIntent};

use tablecraft_core::values::{Currency, Value};
use tablecraft_testkit::fixtures::access::genuine_wide_update::{self as source, Case};

#[path = "wide_indexed_update/recovery.rs"]
mod recovery;
#[path = "wide_indexed_update/resources.rs"]
mod resources;

fn all() -> Vec<Case> {
    source::load_all(|raw, wanted| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, wanted);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn groups(case: &Case, reverse: bool) -> Vec<NativeFixedIntentGroup> {
    let mut values: Vec<_> = case
        .groups
        .iter()
        .map(|(column, updates)| {
            let mut updates: Vec<_> = updates
                .iter()
                .map(|update| {
                    FixedScalarUpdate::new(
                        update.row,
                        source::cell(&update.expected),
                        source::cell(&update.replacement),
                    )
                })
                .collect();
            if reverse {
                updates.reverse();
            }
            NativeFixedIntentGroup::new(column, updates, &mut budget(), &|| false).unwrap()
        })
        .collect();
    if reverse {
        values.reverse();
    }
    values
}

#[test]
#[ignore = "requires all independently qualified wider UPDATE source images"]
fn wider_fixed_original_updates_and_reversed_witness_order_match_complete_literal_images() {
    // Eager loading verifies every original/AFTER pin before the first planner.
    let cases = all();
    assert_eq!(cases.len(), 48);
    for case in cases {
        for reverse in [false, true] {
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    case.table,
                    groups(&case, reverse),
                    &mut budget(),
                    &|| false,
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "{} {} reverse={reverse}: {error:?}",
                        case.table, case.recipe
                    )
                });
            assert_eq!(plan.after_bytes(), case.after);
            if !reverse {
                let raw = plan.intent().encode(&mut budget(), &|| false).unwrap();
                assert_eq!(raw, case.wire);
                let decoded = NativeUpdateIntent::decode(&raw, &mut budget(), &|| false).unwrap();
                assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), case.wire);
            }
            let mut target = Storage::new(case.before.clone());
            let mut log = Storage::new(Vec::new());
            let changed = plan
                .prepare(&mut target, &mut log, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap();
            assert_eq!(changed, case.changed);
            assert_eq!(target.bytes.get_ref(), &case.after);
        }
    }
}

fn primary(case: &Case, maximum: bool) -> Value {
    if !case.table.ends_with("WidePrimary") {
        Value::Int32(if maximum { 3 } else { 1 })
    } else if case.table.starts_with("Currency") {
        Value::Currency(Currency::from_scaled(if maximum {
            i64::MAX
        } else {
            i64::MIN
        }))
    } else {
        Value::Int64(if maximum { i64::MAX } else { i64::MIN })
    }
}

#[test]
#[ignore = "requires independently qualified wider UPDATE originals"]
fn wider_updates_refuse_duplicate_primary_null_primary_stale_and_wrong_carriers() {
    use tablecraft_ace::{
        EditError, IndexConstraintViolation, InspectError, RecoverableEditError, WriteBlocker,
    };
    for case in all()
        .iter()
        .filter(|case| matches!(case.recipe, "joint-marker" | "primary-swap-extremes"))
    {
        for (replacement, violation) in [
            (primary(case, true), IndexConstraintViolation::DuplicateKey),
            (Value::Null, IndexConstraintViolation::NullKey),
        ] {
            let requests = vec![
                NativeFixedIntentGroup::new(
                    "Id",
                    vec![FixedScalarUpdate::new(0, primary(case, false), replacement)],
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            ];
            let result = image(&case.before).plan_recoverable_ordinary_fixed_updates(
                case.table,
                requests,
                &mut budget(),
                &|| false,
            );
            assert!(
                matches!(result, Err(RecoverableEditError::Edit(EditError::Inspect(InspectError::IndexConstraint(actual)))) if actual == violation)
            );
        }
        let expected = if case.table.starts_with("Currency") {
            Value::Currency(Currency::from_scaled(17))
        } else {
            Value::Int64(17)
        };
        let replacement = if case.table.starts_with("Currency") {
            Value::Currency(Currency::from_scaled(0))
        } else {
            Value::Int64(0)
        };
        let requests = vec![
            NativeFixedIntentGroup::new(
                "KeyValue",
                vec![FixedScalarUpdate::new(0, expected, replacement)],
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        ];
        assert!(matches!(
            image(&case.before).plan_recoverable_ordinary_fixed_updates(
                case.table,
                requests,
                &mut budget(),
                &|| false
            ),
            Err(RecoverableEditError::Edit(EditError::Conflict))
        ));

        // Numeric equality never substitutes the original native carrier.
        let wrong = if case.table.starts_with("Currency") {
            Value::Int64(i64::MIN)
        } else {
            Value::Currency(Currency::from_scaled(i64::MIN))
        };
        let requests = vec![
            NativeFixedIntentGroup::new(
                "KeyValue",
                vec![FixedScalarUpdate::new(0, wrong, Value::Null)],
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        ];
        assert!(matches!(
            image(&case.before).plan_recoverable_ordinary_fixed_updates(
                case.table,
                requests,
                &mut budget(),
                &|| false
            ),
            Err(RecoverableEditError::Edit(EditError::Unsupported(
                WriteBlocker::Column
            )))
        ));
    }
}

fn empty_wire(case: &Case) -> Vec<u8> {
    let total = 72 + case.table.len() + 12 + 8;
    let mut raw = Vec::from(*b"TCNI0004");
    raw.extend_from_slice(&(total as u32).to_le_bytes());
    raw.extend_from_slice(&[4, 0, 0, 0]);
    raw.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut binding = Sha256::new();
    binding.update(b"TCNI0001:BEFORE\0");
    binding.update((case.before.len() as u64).to_le_bytes());
    binding.update(&case.before);
    raw.extend_from_slice(&binding.finalize());
    for count in [case.table.len() as u32, 1, 0, 0] {
        raw.extend_from_slice(&count.to_le_bytes());
    }
    raw.extend_from_slice(case.table.as_bytes());
    for count in [8_u32, 0, 0] {
        raw.extend_from_slice(&count.to_le_bytes());
    }
    raw.extend_from_slice(b"KeyValue");
    assert_eq!(raw.len(), total);
    raw
}

#[test]
#[ignore = "requires independently qualified wider UPDATE original source images"]
fn empty_wider_indexed_groups_still_admit_fresh_source_and_retain_original_wire() {
    for case in all()
        .iter()
        .filter(|case| matches!(case.recipe, "joint-marker" | "primary-swap-extremes"))
    {
        let requests = vec![
            NativeFixedIntentGroup::new("KeyValue", Vec::new(), &mut budget(), &|| false).unwrap(),
        ];
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fixed_updates(case.table, requests, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.before);
        assert_eq!(plan.changed_rows(), 0);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            empty_wire(case)
        );
    }
}
