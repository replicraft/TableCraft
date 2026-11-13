//! Complete independent resize oracles and actual cold semantic recovery.
use super::{at, budget, definition, image, retained_image};
use crate::{
    InspectError, NativeImage, NativeQueryCreationIntent, NativeQueryStorageChange,
    QueryCreationError, read_saved_query_definition,
};
use std::io::{Seek, SeekFrom, Write};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{self, RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::saved_queries::creation::{initialization as raw, local_tables},
    support::io::journal::MemoryStorage,
};

#[test]
fn first_query_initializes_exact_storage_and_second_query_uses_existing_storage() {
    for case in [
        raw::Case::Blank,
        raw::Case::Ordinary,
        raw::Case::GeneratedPrimary,
        raw::Case::RequiredText,
    ] {
        let source = raw::source(case, "First", "Answer", 43, at());
        for capture in [image as fn(&[u8]) -> NativeImage, retained_image] {
            let candidate = capture(&source.before)
                .prepare_query_creation(definition("First", "Answer", 43), &mut budget(), &|| false)
                .unwrap_or_else(|error| panic!("{case:?}: {error}"));
            assert_eq!(candidate.before_bytes(), source.before, "{case:?}");
            assert_eq!(candidate.candidate_bytes(), source.expected, "{case:?}");
            assert_eq!(candidate.native_query_id(), i32::MIN + 1);
            let page = u32::try_from(source.before.len() / 4096).unwrap();
            assert_eq!(
                candidate.storage_change(),
                NativeQueryStorageChange::Initialized {
                    native_table_id: i32::try_from(page).unwrap(),
                    definition_page: page,
                }
            );
            let actual = read_saved_query_definition(
                &mut std::io::Cursor::new(candidate.candidate_bytes()),
                CatalogScope::new(911),
                "First",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(actual.rows().len(), 5);
            assert_eq!(actual.rows()[3].name1(), Some("Answer"));
            assert_eq!(actual.rows()[3].expression(), Some("43"));
            let expected =
                local_tables::after(&source.expected, i32::MIN + 2, "Second", "Next", -7, at());
            let next = capture(candidate.candidate_bytes())
                .prepare_query_creation(definition("Second", "Next", -7), &mut budget(), &|| false)
                .unwrap();
            assert_eq!(next.storage_change(), NativeQueryStorageChange::Existing);
            assert_eq!(next.candidate_bytes(), expected);
        }
    }
}

#[test]
fn first_query_resized_original_requalifies_and_recovers_pending_committed_and_settled() {
    for committed in [false, true] {
        let source = raw::source(raw::Case::Ordinary, "Created", "Answer", 99, at());
        let plan = retained_image(&source.before)
            .plan_recoverable_query_creation(
                definition("Created", "Answer", 99),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), source.expected);
        let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
        assert_eq!(&wire[..8], b"TCQC0001");
        NativeQueryCreationIntent::decode(&wire, &mut budget(), &|| false)
            .unwrap()
            .verify_before(&source.before, &mut budget(), &|| false)
            .unwrap();
        let plan = plan
            .requalify_with_budgets(
                retained_image(&source.before),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), source.expected);
        // NO_CLONE: The actual mutable target owns publication/recovery bytes
        // independently of the immutable complete BEFORE and AFTER oracles.
        let mut target = MemoryStorage::new(source.before.clone());
        let mut file = MemoryStorage::new(Vec::new());
        let prepared = plan
            .prepare_retained(&mut target, &mut file, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
        }
        let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
        let outcome = NativeQueryCreationIntent::decode(&wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(912), &mut budget(), &|| false)
            .unwrap()
            .recover_retained(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            outcome,
            if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            }
        );
        assert_eq!(
            target.bytes.get_ref(),
            if committed {
                &source.expected
            } else {
                &source.before
            }
        );
        let writes = target.writes;
        let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
        NativeQueryCreationIntent::decode(&wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(913), &mut budget(), &|| false)
            .unwrap()
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(target.writes, writes);
    }
}

#[test]
fn actual_partial_query_storage_is_terminal_and_never_replaced_by_initialization() {
    let before = raw::partial_query_storage(at());
    let captured = retained_image(&before);
    assert!(
        captured
            .catalog()
            .catalog()
            .objects()
            .iter()
            .any(|object| object.name().as_str() == "MSysQueries")
    );
    assert!(matches!(
        captured
            .prepare_query_creation(definition("First", "Answer", 43), &mut budget(), &|| false),
        Err(QueryCreationError::Source | QueryCreationError::Inspect(_)),
    ));
}

#[test]
fn absent_query_storage_cannot_create_a_user_query_with_the_reserved_storage_name() {
    let source = raw::source(raw::Case::Blank, "First", "Answer", 43, at());
    let captured = retained_image(&source.before);
    assert!(
        captured
            .catalog()
            .catalog()
            .objects()
            .iter()
            .all(|object| object.name().as_str() != "MSysQueries")
    );
    assert!(matches!(
        captured.prepare_query_creation(
            definition("MSysQueries", "Answer", 43),
            &mut budget(),
            &|| false
        ),
        Err(QueryCreationError::Definition),
    ));
}

#[test]
fn missing_query_storage_does_not_waive_existing_user_data_ownership() {
    let source = raw::source(raw::Case::Ordinary, "First", "Answer", 43, at());
    // NO_CLONE: The negative owns its corrupted source while the positive
    // control retains independent complete original and expected images.
    let corrupted = local_tables::foreign_user_data_owner(source.before.clone());
    assert!(matches!(
        retained_image(&corrupted).prepare_query_creation(
            definition("First", "Answer", 43),
            &mut budget(),
            &|| false
        ),
        Err(QueryCreationError::Source | QueryCreationError::Inspect(_)),
    ));
    let control = retained_image(&source.before)
        .prepare_query_creation(definition("First", "Answer", 43), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(control.candidate_bytes(), source.expected);
}

#[test]
fn missing_storage_planning_obeys_cancellation_and_its_callers_actual_pool() {
    let source = raw::source(raw::Case::Ordinary, "First", "Answer", 43, at());
    assert!(matches!(
        retained_image(&source.before).prepare_query_creation(
            definition("First", "Answer", 43),
            &mut budget(),
            &|| true
        ),
        Err(QueryCreationError::Inspect(InspectError::Cancelled)),
    ));
    let mut exhausted = ResourceBudget::new(0, 1_000_000, 500_000_000);
    assert!(matches!(
        retained_image(&source.before).prepare_query_creation(
            definition("First", "Answer", 43),
            &mut exhausted,
            &|| false
        ),
        Err(QueryCreationError::Inspect(InspectError::Limit(_))),
    ));
}

#[test]
fn interrupted_resized_publication_rolls_back_exact_bytes_and_truncates_grown_eof() {
    let source = raw::source(raw::Case::Ordinary, "Created", "Answer", 99, at());
    assert_eq!(source.expected.len(), source.before.len() + 2 * 4096);
    let first_changed = source
        .before
        .iter()
        .zip(&source.expected)
        .position(|(before, after)| before != after)
        .expect("initialization also changes original catalog/allocation bytes");
    let original_prefix = first_changed + 1;
    assert!(
        source.before[original_prefix..]
            .iter()
            .zip(&source.expected[original_prefix..])
            .any(|(before, after)| before != after),
        "the short original-page prefix retains a real BEFORE suffix"
    );

    // Commit writes AFTER from byte zero before final resize/sync/marker. A
    // short write can stop inside an original page, the new TDEF, or new DATA.
    // EOF bytes that are actually present must come from AFTER; absent tails
    // are not invented zero-filled source bytes.
    for published in [
        original_prefix,
        source.before.len() + 1,
        source.before.len() + 4096,
        source.before.len() + 4096 + 17,
    ] {
        let plan = retained_image(&source.before)
            .plan_recoverable_query_creation(
                definition("Created", "Answer", 99),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), source.expected);
        let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
        // NO_CLONE: Each independently interrupted publication owns a mutable
        // target; the original raw images remain unchanged recovery oracles.
        let mut target = MemoryStorage::new(source.before.clone());
        let mut file = MemoryStorage::new(Vec::new());
        drop(
            plan.prepare_retained(&mut target, &mut file, &mut budget(), &|| false)
                .unwrap(),
        );

        target.seek(SeekFrom::Start(0)).unwrap();
        target.write_all(&source.expected[..published]).unwrap();
        assert_eq!(
            target.bytes.get_ref().len(),
            source.before.len().max(published)
        );
        assert_eq!(
            &target.bytes.get_ref()[..published],
            &source.expected[..published]
        );
        assert_ne!(
            target.bytes.get_ref(),
            &source.before,
            "actual interrupted target at {published}"
        );
        assert_ne!(
            target.bytes.get_ref(),
            &source.expected,
            "publication is incomplete at {published}"
        );
        if published > source.before.len() {
            assert_eq!(
                &target.bytes.get_ref()[source.before.len()..],
                &source.expected[source.before.len()..published]
            );
        } else {
            assert_eq!(
                &target.bytes.get_ref()[published..],
                &source.before[published..]
            );
        }
        let published_writes = target.writes;

        let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
        assert_eq!(checked.state(), journal::V3State::Pending);
        let outcome = NativeQueryCreationIntent::decode(&wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(914), &mut budget(), &|| false)
            .unwrap()
            .recover_retained(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(outcome, RecoveryOutcome::RolledBack);
        assert_eq!(target.bytes.get_ref(), &source.before);
        assert_eq!(target.bytes.get_ref().len(), source.before.len());
        assert!(target.writes > published_writes);

        let target_writes = target.writes;
        let journal_writes = file.writes;
        let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
        NativeQueryCreationIntent::decode(&wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(915), &mut budget(), &|| false)
            .unwrap()
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(target.writes, target_writes);
        assert_eq!(file.writes, journal_writes);
        assert_eq!(target.bytes.get_ref(), &source.before);
    }
}
