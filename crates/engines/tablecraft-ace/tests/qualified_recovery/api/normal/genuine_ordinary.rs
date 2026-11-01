//! Intended child of qualified_recovery::api::normal; explicit genuine fixtures.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, FixedScalarUpdate, IndexConstraintViolation, InspectError, NativeFixedIntentGroup,
    NativeImage, NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile,
    RecoverableEditError, qualify_native_update_recovery,
};
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_testkit::access::genuine_ordinary_targets::{self as sources, Case, Recipe};

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn source(header: u8, recipe: Recipe) -> Case {
    let case = sources::source(header, recipe);
    assert_eq!(hash(&case.before), case.before_digest);
    if let Some(after) = &case.after {
        assert_eq!(hash(after), case.after_digest.unwrap());
    }
    case
}

fn image(before: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(before), scope(), &mut budget(), &|| false).unwrap()
}

fn groups(case: &Case) -> Vec<NativeFixedIntentGroup> {
    let updates = if case.empty {
        Vec::new()
    } else {
        vec![FixedScalarUpdate::new(
            0,
            sources::value(case.expected),
            sources::value(case.replacement),
        )]
    };
    vec![NativeFixedIntentGroup::new(case.column, updates, &mut budget(), &|| false).unwrap()]
}

fn carrier(wire: &mut Vec<u8>, value: Option<i32>) {
    wire.push(if value.is_some() { 2 } else { 0 });
    wire.extend_from_slice(
        &value
            .map_or(0, |value| u64::from(value as u32))
            .to_le_bytes(),
    );
}

fn wire(case: &Case) -> Vec<u8> {
    let count = usize::from(!case.empty);
    let total = 72 + 5 + 12 + case.column.len() + count * 26;
    let mut wire = Vec::from(*b"TCNI0004");
    wire.extend_from_slice(&(total as u32).to_le_bytes());
    wire.extend_from_slice(&[4, 0, 0, 0]);
    wire.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    wire.extend_from_slice(&digest.finalize());
    for number in [5_u32, 1, count as u32, 0] {
        wire.extend_from_slice(&number.to_le_bytes());
    }
    wire.extend_from_slice(b"Items");
    for number in [case.column.len() as u32, count as u32, 0] {
        wire.extend_from_slice(&number.to_le_bytes());
    }
    wire.extend_from_slice(case.column.as_bytes());
    if !case.empty {
        wire.extend_from_slice(&0_u64.to_le_bytes());
        carrier(&mut wire, case.expected);
        carrier(&mut wire, case.replacement);
    }
    assert_eq!(wire.len(), total);
    wire
}

fn intent(case: &Case) -> NativeUpdateIntent {
    NativeUpdateIntent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}

#[test]
#[ignore = "requires explicitly supplied source-qualified genuine artifact directory"]
fn ten_genuine_original_outcomes_commit_exact_images_and_original_v4_intents() {
    let mut count = 0;
    for header in [5, 6] {
        for recipe in sources::POSITIVES {
            let case = source(header, recipe);
            let after = case.after.as_ref().unwrap();
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    "Items",
                    groups(&case),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryFixed
            );
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                wire(&case)
            );
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), after);
            assert_eq!(plan.changed_rows(), case.changed_rows);
            let actual = &plan.intent().fixed().unwrap().groups()[0];
            assert_eq!(actual.column(), case.column);
            assert_eq!(actual.updates().len(), usize::from(!case.empty));
            if !case.empty {
                assert_eq!(
                    actual.updates()[0].expected(),
                    &sources::value(case.expected)
                );
                assert_eq!(
                    actual.updates()[0].value(),
                    &sources::value(case.replacement)
                );
            }
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            let prepared = plan
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                prepared.commit(&mut budget(), &|| false).unwrap(),
                case.changed_rows
            );
            assert_eq!(target.bytes.get_ref(), after);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, after, Marker::Committed)
            );
            count += 1;
        }
    }
    assert_eq!(count, 10);
}

#[test]
#[ignore = "requires explicitly supplied source-qualified genuine artifact directory"]
fn genuine_recovery_recaptures_complete_images_in_pending_partial_and_committed_states() {
    for header in [5, 6] {
        for recipe in sources::POSITIVES {
            let case = source(header, recipe);
            let after = case.after.as_ref().unwrap();
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let outcome = if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                };
                let mut target = Storage::new(after.clone());
                let mut journal = Storage::new(journals::journal(&case.before, after, marker));
                let saved = journal.bytes.get_ref().clone();
                for _ in 0..2 {
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_update_recovery(
                        checked,
                        intent(&case),
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(
                        qualified
                            .recover(&mut target, &mut budget(), &|| false)
                            .unwrap(),
                        outcome
                    );
                    assert_eq!(
                        target.bytes.get_ref(),
                        if committed { after } else { &case.before }
                    );
                }
                if committed {
                    assert_eq!(target.writes, 0);
                }
                let writes = target.writes;
                target.fail_prefix = Some(0);
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_update_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    qualified
                        .confirm_completed(&mut target, &mut budget(), &|| false)
                        .unwrap(),
                    outcome
                );
                assert_eq!(target.writes, writes);
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
            }
        }
    }
}

#[test]
#[ignore = "requires explicitly supplied source-qualified genuine artifact directory"]
fn genuine_interrupted_index_data_targets_recover_the_exact_original() {
    for header in [5, 6] {
        let case = source(header, Recipe::ManualIdSeven);
        let after = case.after.as_ref().unwrap();
        let (primary_page, data_page) = if header == 5 { (91, 93) } else { (108, 110) };
        let primary_start = primary_page * 4096;
        let data_start = data_page * 4096;
        let primary_range = primary_start..primary_start + 4096;
        let data_range = data_start..data_start + 4096;

        // Author a target interrupted after the frozen primary INDEX change
        // but before the later DATA page. No plan, writer output, or native
        // encoder supplies this prefix: both images are independent inputs.
        let mut interrupted = case.before.clone();
        interrupted[..data_start].copy_from_slice(&after[..data_start]);
        assert_eq!(
            &interrupted[primary_range.clone()],
            &after[primary_range.clone()]
        );
        assert_ne!(
            &interrupted[primary_range],
            &case.before[primary_start..primary_start + 4096]
        );
        assert_eq!(
            &interrupted[data_range.clone()],
            &case.before[data_range.clone()]
        );
        assert_ne!(
            &interrupted[data_range],
            &after[data_start..data_start + 4096]
        );
        assert_ne!(interrupted, case.before);
        assert_ne!(interrupted, *after);

        for marker in [Marker::Pending, Marker::Partial] {
            let mut target = Storage::new(interrupted.clone());
            let mut journal = Storage::new(journals::journal(&case.before, after, marker));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_update_recovery(
                checked,
                intent(&case),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(target.bytes.get_ref(), &interrupted);
            assert_eq!(target.writes, 0);
            assert_eq!(
                qualified
                    .recover(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                RecoveryOutcome::RolledBack,
            );
            assert_eq!(target.bytes.get_ref(), &case.before);
            assert!(target.writes > 0);
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);

            // Confirmation observes the recovered image with writes denied.
            let writes = target.writes;
            target.fail_prefix = Some(0);
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_update_recovery(
                checked,
                intent(&case),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                qualified
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                RecoveryOutcome::RolledBack,
            );
            assert_eq!(target.bytes.get_ref(), &case.before);
            assert_eq!(target.writes, writes);
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}

#[test]
#[ignore = "requires explicitly supplied source-qualified genuine artifact directory"]
fn genuine_duplicate_and_stale_expectations_return_exact_native_errors() {
    for header in [5, 6] {
        for recipe in [Recipe::DuplicateId, Recipe::StaleScore] {
            let case = source(header, recipe);
            let error = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    "Items",
                    groups(&case),
                    &mut budget(),
                    &|| false,
                )
                .err()
                .expect("the original request is a frozen refusal context");
            if recipe == Recipe::DuplicateId {
                assert!(matches!(
                    error,
                    RecoverableEditError::Edit(EditError::Inspect(InspectError::IndexConstraint(
                        IndexConstraintViolation::DuplicateKey
                    )))
                ));
            } else {
                assert!(matches!(
                    error,
                    RecoverableEditError::Edit(EditError::Conflict)
                ));
            }
        }
    }
}

#[test]
#[ignore = "requires explicitly supplied source-qualified genuine artifact directory"]
fn genuine_recovery_rejects_unrelated_header_index_and_request_forgeries() {
    for header in [5, 6] {
        let case = source(header, Recipe::ScoreSeventeen);
        for variant in 0..3 {
            let mut after = case.after.as_ref().unwrap().clone();
            let original = if variant == 2 {
                let mut changed = source(header, Recipe::ScoreSeventeen);
                changed.replacement = Some(18);
                intent(&changed)
            } else {
                let at = if variant == 0 {
                    200
                } else {
                    (if header == 5 { 92 } else { 109 }) * 4096 + 480
                };
                after[at] ^= 1;
                intent(&case)
            };
            let mut journal =
                Storage::new(journals::journal(&case.before, &after, Marker::Pending));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(checked, original, scope(), &mut budget(), &|| {
                    false
                }),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}
