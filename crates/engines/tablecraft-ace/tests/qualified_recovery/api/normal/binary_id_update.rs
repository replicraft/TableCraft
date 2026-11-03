//! Original TCNI4 Id replacement retains Binary sibling indexes byte-exact.
use super::{Storage, budget, model as journals, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, FixedScalarUpdate, NativeFixedIntentGroup, NativeImage, NativeUpdateIntent,
    RecoverableEditError, WriteBlocker, read_table_index,
};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_binary_id_update::{self as source, Case};

#[path = "binary_id_update/recovery.rs"]
mod recovery;
#[path = "binary_id_update/resources.rs"]
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

fn groups(case: &Case) -> Vec<NativeFixedIntentGroup> {
    request(
        "Id",
        case.updates()
            .into_iter()
            .map(|(row, old, new)| FixedScalarUpdate::new(row, old, new))
            .collect(),
    )
}

fn wire(case: &Case) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    case.wire(digest.finalize().into())
}

#[test]
#[ignore = "requires four independently qualified Binary Id source images"]
fn id_change_noop_and_named_empty_commit_exact_original_request_images() {
    let cases = all();
    assert_eq!(cases.len(), 6);
    for case in cases {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fixed_updates(
                "BinaryKeys",
                groups(&case),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.changed_rows(), case.changed());
        let literal = wire(&case);
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
        case.assert_preserved();
        let saved = image(target.bytes.get_ref());
        for alias in [
            "PrimaryID",
            "PrimaryAlias",
            "Ascending",
            "AscendingAlias",
            "Descending",
            "DescendingAlias",
        ] {
            let observed = read_table_index(
                &mut saved.reader(),
                scope(),
                "BinaryKeys",
                alias,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(observed.rows(), case.index_rows(alias));
        }
    }
}

#[test]
#[ignore = "requires two independently qualified original Binary schemas"]
fn stale_exact_carriers_primary_constraints_and_binary_targets_refuse() {
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == source::Recipe::Changed)
    {
        for (row, old, new, expected) in [
            (0, Value::Int32(9), Value::Int32(14), EditError::Conflict),
            (13, Value::Int32(1), Value::Int32(14), EditError::Conflict),
            (
                0,
                Value::Int64(1),
                Value::Int32(14),
                EditError::Unsupported(WriteBlocker::Column),
            ),
            (
                0,
                Value::Int32(1),
                Value::Int64(14),
                EditError::Unsupported(WriteBlocker::Column),
            ),
        ] {
            assert!(
                matches!(image(&case.before).plan_recoverable_ordinary_fixed_updates(
                "BinaryKeys", request("Id", vec![FixedScalarUpdate::new(row, old, new)]),
                &mut budget(), &|| false,
            ), Err(RecoverableEditError::Edit(actual)) if actual == expected)
            );
        }
        assert!(
            NativeFixedIntentGroup::new(
                "Id",
                vec![
                    FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(14)),
                    FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(14))
                ],
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
        // Required/unique primary constraints, not generated-field restrictions.
        for new in [Value::Null, Value::Int32(2)] {
            assert!(
                image(&case.before)
                    .plan_recoverable_ordinary_fixed_updates(
                        "BinaryKeys",
                        request("Id", vec![FixedScalarUpdate::new(0, Value::Int32(1), new)]),
                        &mut budget(),
                        &|| false,
                    )
                    .is_err()
            );
        }
        // This route changes fixed I32 keys; retained Binary facts confer no target permission.
        for updates in [
            Vec::new(),
            vec![FixedScalarUpdate::new(0, Value::Null, Value::Null)],
        ] {
            assert!(matches!(
                image(&case.before).plan_recoverable_ordinary_fixed_updates(
                    "BinaryKeys",
                    request("KeyValue", updates),
                    &mut budget(),
                    &|| false,
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::Column
                )))
            ));
        }
    }
}
