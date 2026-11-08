use super::*;
use crate::{SaveOperation, SaveReport};
use std::{fs, io::ErrorKind, path::Path};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::journal::{
    CommitError,
    filesystem::{OfflineJournalPair, OfflineTarget},
    prepare_expected,
};
use tablecraft_testkit::support::access::retained_target::assert_retained_target_observation;

type Inspection = fn() -> InspectError;
type WrappedInspection = fn(InspectError) -> Cause;
type WrappedJournal = fn(JournalError) -> Cause;
type InspectionSample = (&'static str, Inspection, Option<SaveFailureKind>);
#[cfg(feature = "ace16-create")]
type SchemaInspection<E> = (&'static str, fn(InspectError) -> E);

fn limit() -> LimitError {
    LimitError {
        resource: Resource::Steps,
        remaining: 0,
    }
}

fn inspections() -> [InspectionSample; 5] {
    [
        (
            "cancelled",
            || InspectError::Cancelled,
            Some(SaveFailureKind::Cancelled),
        ),
        (
            "limited",
            || InspectError::Limit(limit()),
            Some(SaveFailureKind::Resource),
        ),
        (
            "allocation",
            || InspectError::Allocation,
            Some(SaveFailureKind::Resource),
        ),
        (
            "structure",
            || InspectError::StructureLimit,
            Some(SaveFailureKind::Resource),
        ),
        ("malformed", || InspectError::Malformed, None),
    ]
}

#[cfg(feature = "ace16-create")]
fn assert_schema_inspection_layers<E: StdError + 'static>(
    wrappers: &[SchemaInspection<E>],
    cause: fn(E) -> Cause,
    recovery: fn(E) -> NativeRecoveryQualificationError,
) {
    for (layer, wrap) in wrappers {
        for (sample, inspection, expected) in inspections() {
            assert_eq!(
                resource_failure(&wrap(inspection())),
                expected,
                "direct {layer}: {sample}"
            );
            assert_eq!(
                resource_failure(&cause(wrap(inspection()))),
                expected,
                "cause {layer}: {sample}"
            );
            assert_eq!(
                resource_failure(&recovery(wrap(inspection()))),
                expected,
                "recovery {layer}: {sample}"
            );
            assert_eq!(
                resource_failure(&Cause::Qualification(recovery(wrap(inspection())))),
                expected,
                "recovery cause {layer}: {sample}"
            );
        }
    }
}

#[test]
fn restart_open_preserves_transparent_cancellation_and_resource_failures() {
    for (sample, make_error, expected) in inspections() {
        let SaveOutcome::Failed(failure) = failed_open(&OpenError::Native(make_error())) else {
            panic!("an open failure cannot publish a successful outcome");
        };
        assert_eq!(
            failure.kind,
            expected.unwrap_or(SaveFailureKind::Open),
            "{sample}"
        );
    }

    for (error, expected) in [
        (IoError::Cancelled, SaveFailureKind::Cancelled),
        (IoError::Limit(limit()), SaveFailureKind::Resource),
        (
            IoError::Io(ErrorKind::PermissionDenied),
            SaveFailureKind::Open,
        ),
    ] {
        let SaveOutcome::Failed(failure) = failed_open(&OpenError::Io(error)) else {
            panic!("an open failure cannot publish a successful outcome");
        };
        assert_eq!(failure.kind, expected);
    }
}

#[test]
fn row_intent_and_recovery_transparency_preserves_cancel_and_resource_kinds() {
    let wrappers: [(&str, WrappedInspection); 5] = [
        ("native", Cause::Native),
        ("intent", |error| {
            Cause::Intent(NativeIntentError::Inspect(error))
        }),
        ("recovery", |error| {
            Cause::Qualification(NativeRecoveryQualificationError::Inspect(error))
        }),
        ("recovery intent", |error| {
            Cause::Qualification(NativeRecoveryQualificationError::Intent(
                NativeIntentError::Inspect(error),
            ))
        }),
        ("recovery edit", |error| {
            Cause::Qualification(NativeRecoveryQualificationError::Edit(EditError::Inspect(
                error,
            )))
        }),
    ];

    for (layer, wrap) in wrappers {
        for (sample, inspection, expected) in inspections() {
            let cause = wrap(inspection());
            assert_eq!(resource_failure(&cause), expected, "{layer}: {sample}");
        }
    }
}

#[cfg(feature = "ace16-create")]
#[test]
fn first_table_planning_intent_and_recovery_keep_typed_terminal_facts() {
    let wrappers: [(&str, WrappedInspection); 9] = [
        ("planning", |error| {
            Cause::FirstTablePlanning(FirstTableError::Inspect(error))
        }),
        ("planning creation", |error| {
            Cause::FirstTablePlanning(FirstTableError::Create(CreateDatabaseError::Inspect(error)))
        }),
        ("intent", |error| {
            Cause::FirstTableIntent(FirstTableIntentError::Inspect(error))
        }),
        ("intent creation", |error| {
            Cause::FirstTableIntent(FirstTableIntentError::Create(CreateDatabaseError::Inspect(
                error,
            )))
        }),
        ("intent binding", |error| {
            Cause::FirstTableIntent(FirstTableIntentError::Binding(NativeIntentError::Inspect(
                error,
            )))
        }),
        ("recovery planning", |error| {
            Cause::Qualification(NativeRecoveryQualificationError::FirstTable(
                FirstTableError::Inspect(error),
            ))
        }),
        ("recovery creation", |error| {
            Cause::Qualification(NativeRecoveryQualificationError::FirstTable(
                FirstTableError::Create(CreateDatabaseError::Inspect(error)),
            ))
        }),
        ("recovery intent", |error| {
            Cause::Qualification(NativeRecoveryQualificationError::FirstTableIntent(
                FirstTableIntentError::Inspect(error),
            ))
        }),
        ("recovery binding", |error| {
            Cause::Qualification(NativeRecoveryQualificationError::FirstTableIntent(
                FirstTableIntentError::Binding(NativeIntentError::Inspect(error)),
            ))
        }),
    ];

    for (layer, wrap) in wrappers {
        for (sample, inspection, expected) in inspections() {
            let cause = wrap(inspection());
            assert_eq!(resource_failure(&cause), expected, "{layer}: {sample}");
        }
    }
}

#[cfg(feature = "ace16-create")]
#[test]
fn add_table_planning_and_nested_intents_preserve_typed_terminal_facts_at_each_public_layer() {
    assert_schema_inspection_layers(
        &[
            ("planning inspection", AddTableError::Inspect),
            ("planning creation", |error| {
                AddTableError::Create(CreateDatabaseError::Inspect(error))
            }),
        ],
        Cause::AddTablePlanning,
        NativeRecoveryQualificationError::AddTable,
    );
    assert_schema_inspection_layers(
        &[
            ("intent inspection", AddTableIntentError::Inspect),
            ("intent binding", |error| {
                AddTableIntentError::Binding(NativeIntentError::Inspect(error))
            }),
            ("intent binding request", |error| {
                AddTableIntentError::Binding(NativeIntentError::Request(EditError::Inspect(error)))
            }),
            ("declaration inspection", |error| {
                AddTableIntentError::Declaration(FirstTableIntentError::Inspect(error))
            }),
            ("declaration creation", |error| {
                AddTableIntentError::Declaration(FirstTableIntentError::Create(
                    CreateDatabaseError::Inspect(error),
                ))
            }),
            ("declaration binding", |error| {
                AddTableIntentError::Declaration(FirstTableIntentError::Binding(
                    NativeIntentError::Inspect(error),
                ))
            }),
            ("declaration binding request", |error| {
                AddTableIntentError::Declaration(FirstTableIntentError::Binding(
                    NativeIntentError::Request(EditError::Inspect(error)),
                ))
            }),
        ],
        Cause::AddTableIntent,
        NativeRecoveryQualificationError::AddTableIntent,
    );
}

#[cfg(feature = "ace16-create")]
#[test]
fn add_table_profile_capacity_and_integrity_refusals_keep_their_phase_classification() {
    for error in [
        AddTableError::Source,
        AddTableError::DuplicateName,
        AddTableError::Capacity,
        AddTableError::Preservation,
        AddTableError::Create(CreateDatabaseError::Declaration),
    ] {
        assert_eq!(resource_failure(&error), None);
    }
    for error in [
        AddTableIntentError::InvalidRecord,
        AddTableIntentError::UnsupportedVersion,
        AddTableIntentError::UnsupportedProfile,
        AddTableIntentError::BeforeMismatch,
        AddTableIntentError::Declaration(FirstTableIntentError::InvalidRecord),
        AddTableIntentError::Binding(NativeIntentError::BeforeMismatch),
    ] {
        assert_eq!(resource_failure(&error), None);
    }
}

#[test]
fn journal_prepare_and_recovery_classification_does_not_lose_cancel_or_allocation() {
    let wrappers: [(&str, WrappedJournal); 2] =
        [("prepare", Cause::Prepare), ("recovery", Cause::Recovery)];

    for (layer, wrap) in wrappers {
        for (error, expected) in [
            (
                JournalError::Io(IoError::Cancelled),
                Some(SaveFailureKind::Cancelled),
            ),
            (
                JournalError::Io(IoError::Limit(limit())),
                Some(SaveFailureKind::Resource),
            ),
            (JournalError::Allocation, Some(SaveFailureKind::Resource)),
            (
                JournalError::Limit(limit()),
                Some(SaveFailureKind::Resource),
            ),
            (JournalError::InvalidJournal, None),
            (JournalError::TargetMismatch, None),
        ] {
            assert_eq!(resource_failure(&wrap(error)), expected, "{layer}");
        }
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

fn interrupted_commit(root: &Path, cancelled: bool) -> CommitError {
    let path = root.join("target");
    fs::write(&path, b"old").unwrap();
    let target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
    let mut pair = OfflineJournalPair::create(
        target,
        &root.join("journal"),
        b"classifier-only".to_vec(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let error = {
        let (mut target, mut journal) = pair.storage().unwrap();
        let prepared = prepare_expected(
            &mut target,
            &mut journal,
            b"old",
            b"new",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut work = if cancelled {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        prepared.commit(&mut work, &|| cancelled).unwrap_err()
    };
    assert_retained_target_observation(&path, b"old");
    pair.release().unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"old");
    error
}

#[test]
fn actual_commit_error_keeps_its_phase_while_exposing_cancel_or_resource_kind() {
    for cancelled in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let error = Cause::Commit(interrupted_commit(directory.path(), cancelled));
        let expected = if cancelled {
            SaveFailureKind::Cancelled
        } else {
            SaveFailureKind::Resource
        };
        assert!(matches!(error, Cause::Commit(_)));
        assert_eq!(resource_failure(&error), Some(expected));
    }
}

#[test]
fn a_later_recovery_failure_cannot_establish_no_writes_for_the_original_apply() {
    let failure = no_session(SaveFailureKind::InvalidState);
    let apply = SaveReport {
        id: 1,
        operation: SaveOperation::Apply,
        outcome: SaveOutcome::Failed(failure),
    };
    assert!(apply.stopped_before_target_writes());
    for operation in [SaveOperation::Recover, SaveOperation::Release] {
        let later = SaveReport { operation, ..apply };
        assert!(!later.stopped_before_target_writes());
    }
}
