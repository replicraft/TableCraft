//! Original TCNI4, exact wide carriers and unchanged nonnumeric companions.
use super::{Storage, budget, model as journals, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, FixedScalarUpdate, NativeFixedIntentGroup, NativeImage, NativeUpdateIntent,
    RecoverableEditError, WriteBlocker,
};
use tablecraft_core::values::{Currency, Value};
use tablecraft_testkit::fixtures::access::genuine_wide_ordinary_siblings::{
    self as source, Case, Recipe,
};

#[path = "wide_ordinary_siblings/recovery.rs"]
mod recovery;
#[path = "wide_ordinary_siblings/resources.rs"]
mod resources;

fn all() -> Vec<Case> {
    source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn request(column: &str, updates: Vec<FixedScalarUpdate>) -> Vec<NativeFixedIntentGroup> {
    vec![NativeFixedIntentGroup::new(column, updates, &mut budget(), &|| false).unwrap()]
}

fn groups(case: &Case, reverse: bool) -> Vec<NativeFixedIntentGroup> {
    request(
        "KeyValue",
        case.updates(reverse)
            .into_iter()
            .map(|(row, old, new)| FixedScalarUpdate::new(row, old, new))
            .collect(),
    )
}

fn wire(case: &Case, reverse: bool) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    case.wire(digest.finalize().into(), reverse)
}

#[test]
#[ignore = "requires eighteen independent wider-sibling source files"]
fn ordered_original_numeric_requests_commit_exact_companion_preserving_images() {
    let mut commits = 0;
    for case in all() {
        for &reverse in case.orders() {
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    case.table,
                    groups(&case, reverse),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(plan.changed_rows(), case.changed());
            let literal = wire(&case, reverse);
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                literal
            );
            let decoded = NativeUpdateIntent::decode(&literal, &mut budget(), &|| false).unwrap();
            assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), literal);
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            assert_eq!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                case.changed()
            );
            assert_eq!(target.bytes.get_ref(), &case.after);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, &case.after, journals::Marker::Committed)
            );
            let snapshot = tablecraft_ace::read_table(
                &mut Cursor::new(target.bytes.get_ref()),
                scope(),
                case.table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(snapshot.rows(), case.rows());
            case.assert_preserved();
            commits += 1;
        }
    }
    assert_eq!(commits, 28);
}

#[test]
#[ignore = "requires four independently qualified original numeric companion contexts"]
fn original_witnesses_and_actual_numeric_key_and_target_guards_remain_closed() {
    let mut contexts = 0;
    for case in all().into_iter().filter(|case| case.recipe == Recipe::Zero) {
        let wrong = if case.table == "Int64Companions" {
            Value::Currency(Currency::from_scaled(0))
        } else {
            Value::Int64(0)
        };
        for updates in [
            vec![FixedScalarUpdate::new(
                0,
                case.number(i64::MIN + 1),
                case.number(0),
            )],
            vec![FixedScalarUpdate::new(
                3,
                case.number(i64::MIN),
                case.number(0),
            )],
        ] {
            assert!(matches!(
                image(&case.before).plan_recoverable_ordinary_fixed_updates(
                    case.table,
                    request("KeyValue", updates),
                    &mut budget(),
                    &|| false
                ),
                Err(RecoverableEditError::Edit(EditError::Conflict))
            ));
        }
        for (old, new) in [
            (wrong, case.number(0)),
            (case.number(i64::MIN), Value::Int32(0)),
        ] {
            assert!(matches!(
                image(&case.before).plan_recoverable_ordinary_fixed_updates(
                    case.table,
                    request("KeyValue", vec![FixedScalarUpdate::new(0, old, new)]),
                    &mut budget(),
                    &|| false
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::Column
                )))
            ));
        }
        for column in ["Label", "Opaque"] {
            assert!(matches!(
                image(&case.before).plan_recoverable_ordinary_fixed_updates(
                    case.table,
                    request(column, Vec::new()),
                    &mut budget(),
                    &|| false
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::Column
                )))
            ));
        }
        for new in [Value::Null, Value::Int32(2)] {
            assert!(
                image(&case.before)
                    .plan_recoverable_ordinary_fixed_updates(
                        case.table,
                        request("Id", vec![FixedScalarUpdate::new(0, Value::Int32(1), new)]),
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
        }
        assert!(
            NativeFixedIntentGroup::new(
                "KeyValue",
                vec![
                    FixedScalarUpdate::new(0, case.number(i64::MIN), case.number(0)),
                    FixedScalarUpdate::new(0, case.number(i64::MIN), case.number(0)),
                ],
                &mut budget(),
                &|| false
            )
            .is_err()
        );
        contexts += 1;
    }
    assert_eq!(contexts, 4);
}
