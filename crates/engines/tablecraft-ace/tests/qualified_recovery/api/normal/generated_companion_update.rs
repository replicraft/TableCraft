//! Existing TCNI4 producer saves explicit Score edits without generation.
use super::{Storage, budget, model as journals, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, FixedScalarUpdate, NativeFixedIntentGroup, NativeImage, NativeIntentError,
    NativeUpdateIntent, NativeUpdateProfile, RecoverableEditError, WriteBlocker,
};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_generated_companion_update::{
    self as source, Case, Recipe,
};

#[path = "generated_companion_update/recovery.rs"]
mod recovery;
#[path = "generated_companion_update/resources.rs"]
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

fn groups(case: &Case, reverse: bool) -> Vec<NativeFixedIntentGroup> {
    request_group(
        "Score",
        case.updates(reverse)
            .into_iter()
            .map(|(row, old, new)| FixedScalarUpdate::new(row, old, new))
            .collect(),
    )
}

fn request_group(column: &str, updates: Vec<FixedScalarUpdate>) -> Vec<NativeFixedIntentGroup> {
    vec![NativeFixedIntentGroup::new(column, updates, &mut budget(), &|| false).unwrap()]
}

fn wire(case: &Case, reverse: bool) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    case.wire(digest.finalize().into(), reverse)
}

#[test]
#[ignore = "requires twelve independently qualified generated-companion Score source images"]
fn explicit_score_original_order_reverse_null_noop_and_empty_commit_without_generation() {
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
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryFixed
            );
            let literal = wire(&case, reverse);
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                literal
            );
            let original = NativeUpdateIntent::decode(&literal, &mut budget(), &|| false).unwrap();
            assert_eq!(original.encode(&mut budget(), &|| false).unwrap(), literal);
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
            case.assert_preserved();
            commits += 1;
        }
    }
    assert_eq!(commits, 18);
}

#[test]
#[ignore = "requires four independently qualified original generated-companion schemas"]
fn generated_targets_stale_live_ordinals_and_exact_fixed_carrier_guards_remain_closed() {
    let mut originals = 0;
    for case in all().iter().filter(|case| case.recipe == Recipe::Changed) {
        for (row, old, new, expected) in [
            (0, Value::Int32(11), Value::Int32(17), EditError::Conflict),
            (
                case.original_rows().len(),
                Value::Int32(10),
                Value::Int32(17),
                EditError::Conflict,
            ),
            (
                0,
                Value::Int64(10),
                Value::Int32(17),
                EditError::Unsupported(WriteBlocker::Column),
            ),
            (
                0,
                Value::Int32(10),
                Value::Int64(17),
                EditError::Unsupported(WriteBlocker::Column),
            ),
        ] {
            let result = image(&case.before).plan_recoverable_ordinary_fixed_updates(
                case.table,
                request_group("Score", vec![FixedScalarUpdate::new(row, old, new)]),
                &mut budget(),
                &|| false,
            );
            assert!(
                matches!(result, Err(RecoverableEditError::Edit(actual)) if actual == expected)
            );
        }
        for value in [
            Some(Value::Int32(4)),
            Some(Value::Int32(1)),
            Some(Value::Null),
            None,
        ] {
            let updates = value
                .map(|new| FixedScalarUpdate::new(0, Value::Int32(1), new))
                .into_iter()
                .collect();
            let result = image(&case.before).plan_recoverable_ordinary_fixed_updates(
                case.table,
                request_group("Id", updates),
                &mut budget(),
                &|| false,
            );
            assert!(matches!(
                result,
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::Column
                )))
            ));
        }
        let duplicate = NativeFixedIntentGroup::new(
            "Score",
            vec![
                FixedScalarUpdate::new(0, Value::Int32(10), Value::Int32(17)),
                FixedScalarUpdate::new(0, Value::Int32(10), Value::Int32(17)),
            ],
            &mut budget(),
            &|| false,
        );
        assert!(matches!(
            duplicate,
            Err(NativeIntentError::Request(EditError::Conflict))
        ));
        originals += 1;
    }
    assert_eq!(originals, 4);
}
