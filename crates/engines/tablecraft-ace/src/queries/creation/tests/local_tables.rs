//! Actual protected local table sources, using independently authored complete images.
use super::{at, budget, definition, image, retained_image};
use crate::{
    InspectError, NativeImage, NativeQueryCreationIntent, QueryCreationError,
    inspect_database_properties, read_saved_query_definition, read_table,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{self, RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::saved_queries::creation::local_tables::{
    self as raw, Case,
};
use tablecraft_testkit::support::io::journal::MemoryStorage;

#[test]
fn local_rows_generated_primary_history_and_properties_survive_exact_query_creation() {
    for case in [Case::Ordinary, Case::GeneratedPrimary, Case::RequiredText] {
        let source = raw::source(case, "Résumé 新🦀", "Total value", -7, at());
        let before_rows = read_table(
            &mut Cursor::new(&source.before),
            CatalogScope::new(100),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for capture in [
            image as fn(&[u8]) -> NativeImage,
            retained_image as fn(&[u8]) -> NativeImage,
        ] {
            let candidate = capture(&source.before)
                .prepare_query_creation(
                    definition("Résumé 新🦀", "Total value", -7),
                    &mut budget(),
                    &|| false,
                )
                .unwrap_or_else(|error| panic!("{case:?}: {error}"));
            assert_eq!(candidate.before_bytes(), source.before);
            assert_eq!(candidate.candidate_bytes(), source.expected);
            assert_eq!(candidate.native_query_id(), source.native_query_id);
            let rows = read_table(
                &mut Cursor::new(candidate.candidate_bytes()),
                CatalogScope::new(101),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows(), before_rows.rows());
            let created = read_saved_query_definition(
                &mut Cursor::new(candidate.candidate_bytes()),
                CatalogScope::new(101),
                "Résumé 新🦀",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(created.origin().native_id(), source.native_query_id);
            assert_eq!(created.rows().len(), 5);
            assert_eq!(created.rows()[3].expression(), Some("-7"));
            assert_eq!(created.rows()[3].name1(), Some("Total value"));
            for old in ["Saved", "Empty"] {
                let before = read_saved_query_definition(
                    &mut Cursor::new(&source.before),
                    CatalogScope::new(102),
                    old,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let after = read_saved_query_definition(
                    &mut Cursor::new(candidate.candidate_bytes()),
                    CatalogScope::new(102),
                    old,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(before.origin().native_id(), after.origin().native_id());
                assert_eq!(before.rows().len(), after.rows().len());
                for (left, right) in before.rows().iter().zip(after.rows()) {
                    assert_eq!(left.attribute(), right.attribute());
                    assert_eq!(left.expression(), right.expression());
                    assert_eq!(left.name1(), right.name1());
                    assert_eq!(left.flag(), right.flag());
                }
            }
        }
    }
}

#[test]
fn repeated_local_table_creation_and_actual_original_keep_the_same_closed_codec() {
    let source = raw::source(Case::Ordinary, "First", "One", 43, at());
    let plan = retained_image(&source.before)
        .plan_recoverable_query_creation(definition("First", "One", 43), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), source.expected);
    let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
    assert_eq!(&wire[..8], b"TCQC0001");
    let original = NativeQueryCreationIntent::decode(&wire, &mut budget(), &|| false).unwrap();
    original
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
    let expected = raw::after(
        &source.expected,
        source.native_query_id + 1,
        "Second",
        "Two",
        i32::MIN,
        at(),
    );
    let next = retained_image(plan.after_bytes())
        .prepare_query_creation(
            definition("Second", "Two", i32::MIN),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(next.native_query_id(), source.native_query_id + 1);
    assert_eq!(next.candidate_bytes(), expected);
}

#[test]
fn local_table_names_collide_and_foreign_data_or_program_flags_are_terminal_refusals() {
    let source = raw::source(Case::Ordinary, "New", "Answer", 43, at());
    for name in ["Items", "iTeMs", "Saved", "Empty", "MSysQueries"] {
        assert!(
            matches!(
                retained_image(&source.before).prepare_query_creation(
                    definition(name, "Answer", 43),
                    &mut budget(),
                    &|| false
                ),
                Err(QueryCreationError::Definition)
            ),
            "{name}"
        );
    }
    // NO_CLONE: Two independent corruptions need distinct source images while
    // the original complete source stays available for comparison/admission.
    let foreign = raw::foreign_user_data_owner(source.before.clone());
    // NO_CLONE: This independent negative mutates catalog program flags without
    // replacing the immutable ordinary source or the separate owner corruption.
    let program = raw::program_flag(source.before.clone());
    for before in [foreign, program] {
        let captured = retained_image(&before);
        assert!(matches!(
            captured.prepare_query_creation(
                definition("New", "Answer", 43),
                &mut budget(),
                &|| false
            ),
            Err(QueryCreationError::Source | QueryCreationError::Inspect(_))
        ));
    }
    let candidate = retained_image(&source.before)
        .prepare_query_creation(definition("New", "Answer", 43), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.before_bytes(), source.before);
    assert_eq!(candidate.candidate_bytes(), source.expected);
}

#[test]
fn local_table_source_cancellation_and_exhausted_native_pool_never_yield_an_after() {
    let source = raw::source(Case::Ordinary, "New", "Answer", 43, at());
    assert!(matches!(
        retained_image(&source.before).prepare_query_creation(
            definition("New", "Answer", 43),
            &mut budget(),
            &|| true
        ),
        Err(QueryCreationError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        retained_image(&source.before).prepare_query_creation(
            definition("New", "Answer", 43),
            &mut ResourceBudget::new(0, 1_000_000, 500_000_000),
            &|| false
        ),
        Err(QueryCreationError::Inspect(InspectError::Limit(_)))
    ));
}

#[test]
fn user_query_named_msysdb_keeps_old_creation_and_checked_v3_recovery() {
    let source = tablecraft_testkit::fixtures::access::saved_queries::creation::source(
        "MSysDb",
        "Answer",
        -7,
        at(),
    );
    for committed in [false, true] {
        let plan = retained_image(&source.before)
            .plan_recoverable_query_creation(
                definition("MSysDb", "Answer", -7),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), source.expected);
        assert!(
            inspect_database_properties(
                &mut Cursor::new(plan.after_bytes()),
                CatalogScope::new(103),
                &mut budget(),
                &|| false,
            )
            .unwrap()
            .is_none()
        );
        let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
        assert_eq!(&wire[..8], b"TCQC0001");
        let original = NativeQueryCreationIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        // NO_CLONE: Each journal owns mutable publication bytes independently
        // of the unchanged raw BEFORE and AFTER recovery oracles.
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
        let outcome = original
            .qualify_retained_recovery(checked, CatalogScope::new(104), &mut budget(), &|| false)
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
            .qualify_retained_recovery(checked, CatalogScope::new(105), &mut budget(), &|| false)
            .unwrap()
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(target.writes, writes);
    }
}

#[test]
fn malformed_real_database_role_does_not_become_a_user_query_exception() {
    let source = raw::source(Case::GeneratedPrimary, "New", "Answer", -7, at());
    // NO_CLONE: Independent role-type corruption keeps the genuine source
    // available as the immutable control and protected-byte oracle.
    let wrong_type = raw::wrong_database_type(source.before.clone());
    // NO_CLONE: The second negative independently removes the system flags
    // without replacing the first corruption or the genuine source control.
    let wrong_flags = raw::wrong_database_flags(source.before.clone());
    for before in [wrong_type, wrong_flags] {
        assert!(matches!(
            retained_image(&before).prepare_query_creation(
                definition("New", "Answer", -7),
                &mut budget(),
                &|| false,
            ),
            Err(QueryCreationError::Source | QueryCreationError::Inspect(_))
        ));
    }
    let candidate = retained_image(&source.before)
        .prepare_query_creation(definition("New", "Answer", -7), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.candidate_bytes(), source.expected);
}
