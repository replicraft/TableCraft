use super::{Storage, Target, budget, intent, pending, scope};
use std::io::Cursor;
use tablecraft_ace::{
    CreateColumn, FirstTableDefinition, FirstTablePlan, NativeImage,
    NativeRecoveryQualificationError, qualify_native_first_table_recovery,
};
use tablecraft_io::journal::{RecoveryOutcome, inspect_v2};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{
    canonical_blank_creation,
    canonical_creation_properties::{Policy, with_policies},
    canonical_initial_table_creation::{self as fixtures, Case},
};

fn plan(required: bool, allow_zero_length: bool) -> FirstTablePlan {
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::short_text("Name", 255)
            .unwrap()
            .with_required(required)
            .with_allow_zero_length(allow_zero_length)
            .unwrap(),
    ];
    let definition =
        FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| false).unwrap();
    NativeImage::capture(
        &mut Cursor::new(canonical_blank_creation::source()),
        scope(),
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .plan_first_table(definition, &mut budget(), &|| false)
    .unwrap()
}

fn after(required: bool, allow_zero_length: bool) -> Vec<u8> {
    with_policies(
        fixtures::source(Case::ShortText).bytes,
        &[Policy {
            column: "Name",
            required: Some(required),
            allow_zero_length: Some(allow_zero_length),
        }],
    )
}

#[test]
fn policy_v3_reconstructs_independent_complete_after_for_commit_and_pending_recovery() {
    for required in [false, true] {
        for allow in [false, true] {
            let expected = after(required, allow);
            for committed in [false, true] {
                let plan = plan(required, allow);
                assert_eq!(plan.after_bytes(), expected);
                let raw = plan.encode_intent(&mut budget(), &|| false).unwrap();
                assert_eq!(&raw[..8], b"TCNT0003");
                let original = canonical_blank_creation::source();
                let mut target = Target::new(original.clone());
                let mut journal = Storage::new(Vec::new());
                let prepared = plan
                    .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap();
                if committed {
                    prepared.commit(&mut budget(), &|| false).unwrap();
                } else {
                    drop(prepared);
                }
                let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_first_table_recovery(
                    checked,
                    intent(&raw),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    qualified
                        .recover(&mut target, &mut budget(), &|| false)
                        .unwrap(),
                    if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    },
                );
                assert_eq!(
                    target.storage.bytes.get_ref(),
                    if committed { &expected } else { &original },
                );
            }
        }
    }
}

#[test]
fn policy_v3_recovery_refuses_a_changed_policy_intent_or_native_property_without_writes() {
    let original = canonical_blank_creation::source();
    let raw = plan(true, true)
        .encode_intent(&mut budget(), &|| false)
        .unwrap();
    for changed_request in [false, true] {
        let mut requested = raw.clone();
        let replacement = if changed_request {
            // Independent field-header byte: Required is now explicitly false.
            requested[91] &= !4;
            after(true, true)
        } else {
            after(true, false)
        };
        let mut journal = pending(&original, &replacement);
        let writes = journal.writes;
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_first_table_recovery(
                checked,
                intent(&requested),
                scope(),
                &mut budget(),
                &|| false,
            ),
            Err(NativeRecoveryQualificationError::ImageMismatch)
        ));
        assert_eq!(journal.writes, writes);
    }
}
