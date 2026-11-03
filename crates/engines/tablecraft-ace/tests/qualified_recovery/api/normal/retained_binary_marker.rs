//! Public ordinary Marker UPDATE retains complete original Binary index owners.
use super::{Storage, budget, model as journals, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, FixedScalarUpdate, NativeFixedIntentGroup, NativeImage, NativeIntentError,
    NativeUpdateIntent, NativeUpdateProfile, RecoverableEditError, WriteBlocker,
};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_retained_binary_marker::{
    self as source, Case, Recipe,
};

#[path = "retained_binary_marker/recovery.rs"]
mod recovery;
#[path = "retained_binary_marker/resources.rs"]
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
    vec![
        NativeFixedIntentGroup::new(
            "Marker",
            case.updates(reverse)
                .into_iter()
                .map(|(row, old, new)| FixedScalarUpdate::new(row, old, new))
                .collect(),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn wire(case: &Case, reverse: bool) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    case.wire(digest.finalize().into(), reverse)
}

#[test]
#[ignore = "requires six independently qualified retained Binary Marker images"]
fn marker_original_batch_reverse_null_noop_and_named_empty_commit_exact_images() {
    let cases = all();
    let mut commits = 0;
    for case in &cases {
        let orders: &[bool] = if case.recipe == Recipe::Batch {
            &[false, true]
        } else {
            &[false]
        };
        for &reverse in orders {
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    "BinaryKeys",
                    groups(case, reverse),
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
            let expected = wire(case, reverse);
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                expected
            );
            let decoded = NativeUpdateIntent::decode(&expected, &mut budget(), &|| false).unwrap();
            assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), expected);
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            let committed = plan
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap();
            assert_eq!(committed, case.changed());
            assert_eq!(target.bytes.get_ref(), &case.after);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, &case.after, journals::Marker::Committed,)
            );
            case.assert_complement();
            commits += 1;
        }
    }
    assert_eq!(commits, 10);
}

fn request(column: &str, row: usize, old: Value, new: Value) -> Vec<NativeFixedIntentGroup> {
    vec![
        NativeFixedIntentGroup::new(
            column,
            vec![FixedScalarUpdate::new(row, old, new)],
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

#[test]
#[ignore = "requires independently qualified original Binary schemas"]
fn marker_stale_exact_carrier_and_duplicate_boundaries_remain_closed() {
    let mut originals = 0;
    for case in all().iter().filter(|case| case.recipe == Recipe::Batch) {
        for (row, old, new, expected) in [
            (0, Value::Int32(18), Value::Int32(29), EditError::Conflict),
            (13, Value::Int32(17), Value::Int32(29), EditError::Conflict),
            (
                0,
                Value::Int64(17),
                Value::Int32(29),
                EditError::Unsupported(WriteBlocker::Column),
            ),
            (
                0,
                Value::Int32(17),
                Value::Int64(29),
                EditError::Unsupported(WriteBlocker::Column),
            ),
        ] {
            let result = image(&case.before).plan_recoverable_ordinary_fixed_updates(
                "BinaryKeys",
                request("Marker", row, old, new),
                &mut budget(),
                &|| false,
            );
            assert!(
                matches!(result, Err(RecoverableEditError::Edit(actual)) if actual == expected)
            );
        }
        // Primary-key updates now have separate literal source, proof and save
        // qualification in binary_id_update; retain Marker-specific guards here.
        let duplicate = NativeFixedIntentGroup::new(
            "Marker",
            vec![
                FixedScalarUpdate::new(0, Value::Int32(17), Value::Int32(29)),
                FixedScalarUpdate::new(0, Value::Int32(17), Value::Int32(29)),
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
    assert_eq!(originals, 2);
}
