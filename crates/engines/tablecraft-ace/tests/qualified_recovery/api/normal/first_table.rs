//! First-table schema effects reuse this feature's fixed/resizable fault storage.
use super::{Storage, budget, resizable_storage::Target, scope};
use std::io::Cursor;
use tablecraft_ace::{
    CreateColumn, FirstTableDefinition, FirstTablePlan, NativeFirstTableIntent, NativeImage,
    NativeRecoveryQualificationError, qualify_native_first_table_recovery,
};
use tablecraft_io::journal::{RecoveryOutcome, V2State, inspect_v2, prepare_resizable_expected};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{
    canonical_blank_creation, canonical_initial_table_creation,
};

fn plan() -> FirstTablePlan {
    let before = canonical_blank_creation::source();
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::short_text("Name", 255).unwrap(),
    ];
    let definition =
        FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| false).unwrap();
    NativeImage::capture(&mut Cursor::new(before), scope(), &mut budget(), &|| false)
        .unwrap()
        .plan_first_table(definition, &mut budget(), &|| false)
        .unwrap()
}

fn intent(raw: &[u8]) -> NativeFirstTableIntent {
    NativeFirstTableIntent::decode(raw, &mut budget(), &|| false).unwrap()
}

fn pending(before: &[u8], after: &[u8]) -> Storage {
    let mut target = Target::new(before.to_vec());
    let mut journal = Storage::new(Vec::new());
    drop(
        prepare_resizable_expected(
            &mut target,
            &mut journal,
            before,
            after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    assert_eq!(target.storage.writes, 0);
    journal
}

#[test]
fn first_table_prepares_and_commits_only_the_exact_retained_blank_target() {
    let before = canonical_blank_creation::source();
    let after =
        canonical_initial_table_creation::source(canonical_initial_table_creation::Case::ShortText)
            .bytes;
    let mut target = Target::new(before.clone());
    let mut journal = Storage::new(Vec::new());
    let prepared = plan()
        .prepare(&mut target, &mut journal, &mut budget(), &|| false)
        .unwrap();
    prepared.commit(&mut budget(), &|| false).unwrap();
    assert_eq!(target.storage.bytes.get_ref(), &after);
    assert!(target.resizes > 0);

    let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
    assert_eq!(checked.state(), V2State::Committed);
    let raw = plan().encode_intent(&mut budget(), &|| false).unwrap();
    let qualified =
        qualify_native_first_table_recovery(checked, intent(&raw), scope(), &mut budget(), &|| {
            false
        })
        .unwrap();
    let writes = target.storage.writes;
    assert_eq!(
        qualified
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap(),
        RecoveryOutcome::Committed
    );
    assert_eq!(target.storage.writes, writes);

    let mut wrong = before;
    *wrong.last_mut().unwrap() = 31;
    let mut target = Target::new(wrong.clone());
    let mut empty = Storage::new(Vec::new());
    assert!(
        plan()
            .prepare(&mut target, &mut empty, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(target.storage.bytes.get_ref(), &wrong);
    assert_eq!(target.storage.writes, 0);
    assert_eq!(empty.writes, 0);
}

#[test]
fn first_table_interrupted_v2_commit_requires_fresh_qualification_before_rollback() {
    let before = canonical_blank_creation::source();
    let mut target = Target::new(before.clone());
    let mut journal = Storage::new(Vec::new());
    let raw = plan().encode_intent(&mut budget(), &|| false).unwrap();
    target.storage.fail_prefix = Some(35_000);
    assert!(
        plan()
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap()
            .commit(&mut budget(), &|| false)
            .is_err()
    );
    assert_ne!(target.storage.bytes.get_ref(), &before);
    assert!(target.storage.bytes.get_ref().len() > before.len());

    let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
    assert_eq!(checked.state(), V2State::Pending);
    let qualified =
        qualify_native_first_table_recovery(checked, intent(&raw), scope(), &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(
        qualified
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap(),
        RecoveryOutcome::RolledBack
    );
    assert_eq!(target.storage.bytes.get_ref(), &before);
}

#[test]
fn first_table_recovery_refuses_changed_requests_and_unqualified_after_images_without_writes() {
    let before = canonical_blank_creation::source();
    let after =
        canonical_initial_table_creation::source(canonical_initial_table_creation::Case::ShortText)
            .bytes;
    let raw = plan().encode_intent(&mut budget(), &|| false).unwrap();
    for changed_request in [false, true] {
        let mut requested = raw.clone();
        let mut replacement = after.clone();
        if changed_request {
            // The final original field spelling changes but remains well-formed.
            *requested.last_mut().unwrap() = b'x';
        } else {
            *replacement.last_mut().unwrap() ^= 1;
        }
        let mut journal = pending(&before, &replacement);
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

#[path = "first_table/policies.rs"]
mod policies;
