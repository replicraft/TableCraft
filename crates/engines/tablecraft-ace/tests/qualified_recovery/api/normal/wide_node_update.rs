//! Source-pinned existing-tree UPDATE plans and original TCNI4 requests.
use super::{Storage, budget, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{FixedScalarUpdate, NativeFixedIntentGroup, NativeImage, NativeUpdateIntent};
use tablecraft_core::values::Value;
use tablecraft_testkit::access::genuine_wide_node_update::{self as source, Case, Recipe};

#[path = "wide_node_update/recovery.rs"]
mod recovery;
#[path = "wide_node_update/resources.rs"]
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
    let mut groups: Vec<_> = source::groups(case)
        .into_iter()
        .map(|(column, assignments)| {
            let mut updates: Vec<_> = assignments
                .into_iter()
                .map(|assignment| {
                    FixedScalarUpdate::new(
                        assignment.row,
                        assignment.expected,
                        assignment.replacement,
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
        groups.reverse();
    }
    groups
}

fn carrier(value: &Value) -> [u8; 9] {
    let mut raw = [0; 9];
    match value {
        Value::Int32(value) => {
            raw[0] = 2;
            raw[1..5].copy_from_slice(&value.to_le_bytes());
        }
        Value::Int64(value) => {
            raw[0] = 3;
            raw[1..].copy_from_slice(&value.to_le_bytes());
        }
        Value::Currency(value) => {
            raw[0] = 4;
            raw[1..].copy_from_slice(&value.scaled().to_le_bytes());
        }
        _ => panic!("outside independently frozen present numeric requests"),
    }
    raw
}

fn original_wire(case: &Case) -> Vec<u8> {
    // Independent protocol law: original ordered rows, 9-byte carriers and a
    // domain-separated digest of the complete immutable BEFORE image.
    let groups = source::groups(case);
    let count: usize = groups.iter().map(|(_, rows)| rows.len()).sum();
    let total = 72
        + case.table.len()
        + groups
            .iter()
            .map(|(name, rows)| 12 + name.len() + 26 * rows.len())
            .sum::<usize>();
    let mut raw = Vec::from(*b"TCNI0004");
    raw.extend_from_slice(&(total as u32).to_le_bytes());
    raw.extend_from_slice(&[4, 0, 0, 0]);
    raw.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    raw.extend_from_slice(&digest.finalize());
    for field in [
        case.table.len() as u32,
        groups.len() as u32,
        count as u32,
        0,
    ] {
        raw.extend_from_slice(&field.to_le_bytes());
    }
    raw.extend_from_slice(case.table.as_bytes());
    for (name, rows) in groups {
        for field in [name.len() as u32, rows.len() as u32, 0] {
            raw.extend_from_slice(&field.to_le_bytes());
        }
        raw.extend_from_slice(name.as_bytes());
        for row in rows {
            raw.extend_from_slice(&(row.row as u64).to_le_bytes());
            raw.extend_from_slice(&carrier(&row.expected));
            raw.extend_from_slice(&carrier(&row.replacement));
        }
    }
    assert_eq!(raw.len(), total);
    raw
}

fn forged_offsets(case: &Case) -> Vec<usize> {
    let mut offsets = vec![
        case.facts.definition as usize * 4096 + 16,
        case.facts.definition as usize * 4096 + 67,
        case.facts.records[199].offset + 18,
        case.facts.records[199].offset + 6,
        4096 + 100,
    ];
    for root in case.facts.roots {
        let base = root as usize * 4096;
        offsets.extend([base + 2, base + 27, base + 480, base + 492, base + 4095]);
    }
    for child in case.facts.children.into_iter().flatten() {
        let base = child as usize * 4096;
        offsets.extend([base + 8, base + 27, base + 480, base + 489, base + 4095]);
    }
    offsets
}

#[test]
#[ignore = "requires qualified retained numeric NODE UPDATE sources"]
fn retained_numeric_updates_match_literal_images_in_both_original_orders_and_commit() {
    let cases = all();
    assert_eq!(cases.len(), 20);
    for case in cases {
        for reverse in [false, true] {
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    case.table,
                    groups(&case, reverse),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.after_bytes(), case.after);
            if !reverse {
                let raw = plan.intent().encode(&mut budget(), &|| false).unwrap();
                assert_eq!(raw, original_wire(&case));
                let decoded = NativeUpdateIntent::decode(&raw, &mut budget(), &|| false).unwrap();
                assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), raw);
            }
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            let changed = plan
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap();
            assert_eq!(changed, source::changed_rows(case.recipe));
            assert_eq!(target.bytes.get_ref(), &case.after);
        }
    }
}

#[test]
#[ignore = "requires qualified retained numeric NODE originals"]
fn retained_numeric_empty_requests_stale_witnesses_and_exact_carriers_are_checked() {
    use tablecraft_ace::{EditError, IndexConstraintViolation, InspectError, RecoverableEditError};
    for case in all().iter().filter(|case| case.recipe == Recipe::Joint) {
        let empty =
            NativeFixedIntentGroup::new("KeyValue", Vec::new(), &mut budget(), &|| false).unwrap();
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fixed_updates(
                case.table,
                vec![empty],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), case.before);
        let wrong = if case.facts.money {
            Value::Int64(0)
        } else {
            Value::Currency(tablecraft_core::values::Currency::from_scaled(0))
        };
        let update = FixedScalarUpdate::new(199, wrong, Value::Null);
        let group = NativeFixedIntentGroup::new("KeyValue", vec![update], &mut budget(), &|| false)
            .unwrap();
        assert!(matches!(
            image(&case.before).plan_recoverable_ordinary_fixed_updates(
                case.table,
                vec![group],
                &mut budget(),
                &|| false
            ),
            Err(RecoverableEditError::Edit(EditError::Unsupported(
                tablecraft_ace::WriteBlocker::Column
            )))
        ));
        for (expected, replacement, stale) in [
            (Value::Null, Value::Null, true),
            (
                if case.facts.money {
                    Value::Currency(tablecraft_core::values::Currency::from_scaled(0))
                } else {
                    Value::Int64(0)
                },
                Value::Null,
                false,
            ),
        ] {
            let update = FixedScalarUpdate::new(199, expected, replacement);
            let group =
                NativeFixedIntentGroup::new("KeyValue", vec![update], &mut budget(), &|| false)
                    .unwrap();
            let result = image(&case.before).plan_recoverable_ordinary_fixed_updates(
                case.table,
                vec![group],
                &mut budget(),
                &|| false,
            );
            if stale {
                assert!(matches!(
                    result,
                    Err(RecoverableEditError::Edit(EditError::Conflict))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(RecoverableEditError::Edit(EditError::Inspect(
                        InspectError::IndexConstraint(IndexConstraintViolation::NullKey)
                    )))
                ));
            }
        }
    }
}
