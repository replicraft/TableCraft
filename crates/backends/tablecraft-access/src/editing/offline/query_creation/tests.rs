//! Real source requalification, query-only receipts and fresh TCQC recovery.
use super::*;
use crate::editing::offline::{
    CompletionKind,
    plan::MutationIntent,
    test_support::{assert_staged_read_state, retained_schema_recovery::cold_case},
};
use crate::{SavedQueryCreationOptions, prepare_saved_query_creation};
use std::{fs, io::Cursor};
use tablecraft_ace::{
    InspectError, NativeImage, NativeRecoveryQualificationError, QueryCreationIntentError,
};
use tablecraft_io::journal::RecoveryOutcome;
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::saved_queries::creation as raw;

const SCOPE: CatalogScope = CatalogScope::new(1931);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn at() -> LegacyDateTime {
    LegacyDateTime::from_ole_days(45000.5).unwrap()
}

fn plan(source: &raw::Source) -> RecoverableNativeQueryCreationPlan {
    let image = NativeImage::capture_retained(
        &mut Cursor::new(&source.before),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let parsed = parse_query("SELECT -7 AS [Total value]", ParseLimits::default()).unwrap();
    prepare_saved_query_creation(
        image,
        parsed.query(),
        SavedQueryCreationOptions {
            destination: "Created",
            captured_at: at(),
            limits: ParseLimits::default(),
        },
        &mut budget(),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn stage_apply_reports_only_actual_query_creation_and_preserves_full_independent_after() {
    let source = raw::source("Created", "Total value", -7, at());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("query.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &source.before).unwrap();
    let staged = open(&path)
        .stage_reviewed_query_creation_with_budgets(
            plan(&source),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(staged.name(), "Created");
    assert_eq!(staged.alias(), "Total value");
    assert_eq!(staged.value(), -7);
    assert_eq!(staged.captured_at(), at());
    assert_eq!(staged.native_query_id(), source.native_query_id);
    let wire = staged
        .plan
        .intent()
        .encode(&mut budget(), &|| false)
        .unwrap();
    assert!(wire.starts_with(b"TCQC0001"));
    let decoded = MutationIntent::decode(&wire, &mut budget(), &|| false).unwrap();
    assert!(matches!(decoded, MutationIntent::QueryCreation(_)));
    assert_staged_read_state(&path, &source.before);
    assert!(!bundle.exists());

    let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
    assert_eq!(receipt.kind(), CompletionKind::Applied);
    assert_eq!(receipt.query_change(), Some(QueryChange::Created));
    assert_eq!(receipt.schema_change(), None);
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    owner.release().unwrap();
    assert_eq!(fs::read(path).unwrap(), source.expected);
    assert!(!bundle.exists());
}

#[test]
fn discarded_staging_retains_its_owner_without_a_query_or_journal() {
    let source = raw::source("Created", "Total value", -7, at());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("query.accdb");
    fs::write(&path, &source.before).unwrap();
    let staged = open(&path)
        .stage_reviewed_query_creation_with_budgets(
            plan(&source),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    staged.into_session().release().unwrap();
    assert_eq!(fs::read(path).unwrap(), source.before);
    assert!(!directory.path().join("journal").exists());
}

#[test]
fn cold_pending_committed_and_settled_use_only_original_tcqc_and_query_receipts() {
    let source = raw::source("Created", "Total value", -7, at());
    for committed in [false, true] {
        for retired in [false, true] {
            let original = plan(&source);
            let encoded = original.intent().encode(&mut budget(), &|| false).unwrap();
            cold_case(
                &source.before,
                &source.expected,
                encoded,
                committed,
                retired,
                QueryChange::Created,
                |pair, committed, retired| {
                    let (mut target, mut journal) = pair.resizable_storage().unwrap();
                    let prepared = original
                        .prepare_retained(&mut target, &mut journal, &mut budget(), &|| false)
                        .unwrap();
                    if committed {
                        prepared.commit(&mut budget(), &|| false).unwrap();
                    } else if retired {
                        prepared.rollback(&mut budget(), &|| false).unwrap();
                    }
                },
            );
        }
    }
}

#[test]
fn changed_source_cancel_and_both_resource_pools_refuse_before_namespace_and_retain_owner() {
    let source = raw::source("Created", "Total value", -7, at());
    for case in 0..4 {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("query.accdb");
        let actual = if case == 0 {
            &source.expected
        } else {
            &source.before
        };
        fs::write(&path, actual).unwrap();
        let mut native = if case == 2 {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let mut proof = if case == 3 {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let failure = match open(&path).stage_reviewed_query_creation_with_budgets(
            plan(&source),
            &mut native,
            &mut proof,
            &|| case == 1,
        ) {
            Err(failure) => failure,
            Ok(_) => panic!("stale, cancelled or exhausted creation review must refuse"),
        };
        match case {
            0 => assert!(matches!(failure.cause(), Cause::SourceChanged)),
            1 => assert!(matches!(
                failure.cause(),
                Cause::Native(InspectError::Cancelled)
            )),
            2 => assert!(matches!(
                failure.cause(),
                Cause::Native(InspectError::Limit(_))
            )),
            3 => assert!(matches!(
                failure.cause(),
                Cause::QueryCreationIntent(QueryCreationIntentError::Proof(
                    NativeRecoveryQualificationError::Inspect(InspectError::Limit(_))
                ))
            )),
            _ => unreachable!(),
        }
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(path).unwrap(), *actual);
        assert!(!directory.path().join("journal").exists());
    }
}

#[test]
fn exact_family_dispatch_never_retries_other_query_intents_for_an_unknown_version() {
    let source = raw::source("Created", "Total value", -7, at());
    let mut wire = plan(&source)
        .intent()
        .encode(&mut budget(), &|| false)
        .unwrap();
    wire[7] = b'2';
    assert!(matches!(
        MutationIntent::decode(&wire, &mut budget(), &|| false),
        Err(Cause::QueryCreationIntent(
            QueryCreationIntentError::UnsupportedVersion
        ))
    ));
}
