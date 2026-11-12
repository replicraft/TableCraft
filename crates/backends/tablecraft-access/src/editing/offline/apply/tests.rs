//! A real large retained native plan crosses namespace setup and paid reopening.
use super::super::{
    CompletionKind, OfflineWriteSession, Phase, SessionState, StagedUpdate, plan::MutationPlan,
    test_support::assert_completion,
};
use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{
    FixedScalarUpdate, NativeFittedIntentGroup, NativeFittedUpdates, NativeImage,
    NativeUpdateIntent, NativeUpdateProfile, RecoverableNativeEditPlan,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_io::{
    IoError, SharedFile,
    journal::{
        RecoveryOutcome,
        filesystem::{OfflineJournalPair, PairError},
    },
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{fields, populated};

const ROWS: usize = 470_000;
const WIRE_BOUNDARY: usize = 16 * 1024 * 1024;
const SCOPE: CatalogScope = CatalogScope::new(172_527);

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024 * 1024, 64_000_000, 200_000_000_000)
}

fn source() -> Vec<u8> {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    populated::numbers(&seed, "LargeValues", &vec![Some(1); ROWS])
}

fn plan(before: &[u8], work: &mut ResourceBudget) -> RecoverableNativeEditPlan {
    let image =
        NativeImage::capture_retained(&mut Cursor::new(before), SCOPE, work, &|| false).unwrap();
    work.charge_allocation(ROWS as u64, size_of::<FixedScalarUpdate>() as u64)
        .unwrap();
    let updates = (0..ROWS)
        .map(|row| FixedScalarUpdate::new(row, Value::Int32(1), Value::Int32(1)))
        .collect();
    let group = NativeFittedIntentGroup::new_retained_long_column_batch(
        "CopiedScore",
        NativeFittedUpdates::Fixed(updates),
        work,
        &|| false,
    )
    .unwrap();
    let plan = image
        .plan_recoverable_retained_long_column_batch_updates(
            "LargeValues",
            vec![group],
            work,
            &|| false,
        )
        .unwrap();
    assert_eq!(
        plan.intent().profile(),
        NativeUpdateProfile::Ace16RetainedLongColumnBatch
    );
    assert_eq!(plan.changed_rows(), 0);
    equal_image(plan.before_bytes(), before);
    equal_image(plan.after_bytes(), before);
    plan
}

fn open(path: &Path, work: &mut ResourceBudget) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, work, &|| false).unwrap()
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

fn assert_held(path: &Path) {
    assert!(matches!(SharedFile::open(path), Err(IoError::LockBusy)));
}

fn namespace(bundle: &Path) -> [Vec<u8>; 3] {
    ["binding", "intent", "journal"].map(|name| fs::read(bundle.join(name)).unwrap())
}

fn assert_namespace(bundle: &Path, expected: &[Vec<u8>; 3]) {
    for (name, expected) in ["binding", "intent", "journal"].into_iter().zip(expected) {
        equal_image(&fs::read(bundle.join(name)).unwrap(), expected);
    }
}

#[test]
#[ignore = "explicit release boundary: a real 470000-row native plan and retained original"]
fn large_retained_native_plan_applies_and_reopens_with_paid_limits_and_cancel_ownership() {
    let before = source();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("large.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &before).unwrap();
    let mut work = budget();

    // This sealed candidate comes from complete native source/record proof,
    // not from a data-only original or a constructed query-result capability.
    let candidate = plan(&before, &mut work);
    let wire = candidate.intent().encode(&mut work, &|| false).unwrap();
    assert!(wire.len() > WIRE_BOUNDARY);
    let decoded = NativeUpdateIntent::decode(&wire, &mut work, &|| false).unwrap();
    decoded
        .verify_before(&before, &mut work, &|| false)
        .unwrap();
    assert_eq!(
        decoded.profile(),
        NativeUpdateProfile::Ace16RetainedLongColumnBatch
    );
    drop(decoded);
    drop(wire);

    let staged = StagedUpdate::from_plan(open(&path, &mut work), candidate, ROWS);
    assert_eq!(staged.matched_rows(), ROWS);
    assert_eq!(staged.changed_rows(), 0);
    let (owner, receipt) = staged.apply(&bundle, &mut work, &|| false).unwrap();
    assert_eq!(receipt.kind(), CompletionKind::Applied);
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (Some(ROWS), Some(0))
    );
    assert!(receipt.schema_change().is_none());
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    assert_completion(&receipt);
    owner.release().unwrap();
    equal_image(&fs::read(&path).unwrap(), &before);
    assert!(!bundle.exists());

    // A second genuine native candidate establishes a committed V3 pair whose
    // equal images do not themselves prove which journal outcome occurred.
    let candidate = plan(&before, &mut work);
    let action = MutationPlan::Update(candidate);
    let wire = action.encode_intent(&mut work, &|| false).unwrap();
    assert!(wire.len() > WIRE_BOUNDARY);
    let limit = action.explicit_intent_limit(wire.len()).unwrap().unwrap();
    assert_eq!(limit, wire.len() as u64);
    let MutationPlan::Update(candidate) = action else {
        panic!("actual selected native UPDATE");
    };
    let mut owner = open(&path, &mut work);
    {
        let slot = owner.slot_mut().unwrap();
        let target = slot.target.take().unwrap();
        let mut pair = OfflineJournalPair::create_with_intent_limit(
            target,
            &bundle,
            wire,
            limit,
            &mut work,
            &|| false,
        )
        .unwrap();
        {
            let (mut target, mut journal) = pair.resizable_storage().unwrap();
            candidate
                .prepare_retained(&mut target, &mut journal, &mut work, &|| false)
                .unwrap()
                .commit(&mut work, &|| false)
                .unwrap();
        }
        slot.pair = Some(pair);
        slot.state = SessionState::RetainedJournal;
    }
    owner.release().unwrap();
    let retained = namespace(&bundle);
    equal_image(&fs::read(&path).unwrap(), &before);

    // Existing-pair association pays the actual intent allocation before read.
    let owner = open(&path, &mut work);
    let mut too_small = ResourceBudget::new(WIRE_BOUNDARY as u64, 1_000_000, 1_000_000_000);
    let failure = match owner.recover_update(&bundle, &mut too_small, &|| false) {
        Err(failure) => failure,
        Ok(_) => panic!("paid intent reopening must refuse the insufficient byte budget"),
    };
    assert_eq!(failure.phase(), Phase::Setup);
    assert!(
        matches!(failure.setup_failure().unwrap().cause(), PairError::Io(IoError::Limit(error)) if error.resource == Resource::Bytes)
    );
    assert!(failure.current_outcome().is_none());
    assert_held(&path);
    failure.into_session().release().unwrap();
    equal_image(&fs::read(&path).unwrap(), &before);
    assert_namespace(&bundle, &retained);

    let owner = open(&path, &mut work);
    let failure = match owner.recover_update(&bundle, &mut work, &|| true) {
        Err(failure) => failure,
        Ok(_) => panic!("cancelled reopening must retain the unmodified source and namespace"),
    };
    assert_eq!(failure.phase(), Phase::Setup);
    assert!(matches!(
        failure.setup_failure().unwrap().cause(),
        PairError::Io(IoError::Cancelled)
    ));
    assert!(failure.current_outcome().is_none());
    assert_held(&path);
    failure.into_session().release().unwrap();
    equal_image(&fs::read(&path).unwrap(), &before);
    assert_namespace(&bundle, &retained);

    let (owner, receipt) = open(&path, &mut work)
        .recover_update(&bundle, &mut work, &|| false)
        .unwrap();
    assert_eq!(receipt.kind(), CompletionKind::Recovered);
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    assert!(receipt.schema_change().is_none());
    assert_completion(&receipt);
    owner.release().unwrap();
    equal_image(&fs::read(path).unwrap(), &before);
    assert!(!bundle.exists());
}
