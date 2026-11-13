//! Boundary evidence belongs to the qualified reader, not u8 representability alone.
use super::super::{at, budget, definition, retained_image};
use crate::{
    NativeQueryCreationIntent, NativeQueryStorageChange, QueryCreationError,
    read_saved_query_definition,
};
use tablecraft_io::journal::{self, RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::saved_queries::creation::initialization::map_boundary::{self as raw, Hosts},
    support::io::journal::MemoryStorage,
};

fn rows(bytes: &[u8], page: usize) -> u16 {
    u16::from_le_bytes(
        bytes[page * 4096 + 12..page * 4096 + 14]
            .try_into()
            .unwrap(),
    )
}

#[test]
fn last_admitted_host_preserves_all_existing_maps_and_produces_readable_complete_after() {
    let source = raw::source(251, Hosts::Only, "First", "Answer", 43, at());
    assert_eq!(rows(&source.before, 3), 251);
    assert_eq!(rows(&source.expected, 3), 255);
    let candidate = retained_image(&source.before)
        .prepare_query_creation(definition("First", "Answer", 43), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.before_bytes(), source.before);
    assert_eq!(candidate.candidate_bytes(), source.expected);
    let query = read_saved_query_definition(
        &mut std::io::Cursor::new(candidate.candidate_bytes()),
        CatalogScope::new(917),
        "First",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(query.rows().len(), 5);
    assert_eq!(query.rows()[3].expression(), Some("43"));
}

#[test]
fn an_overfull_first_host_is_refused_or_skipped_for_a_real_referenced_alternate_host() {
    for existing_rows in [252, 253] {
        let before = raw::before(existing_rows, Hosts::Only);
        assert_eq!(rows(&before, 3), existing_rows as u16);
        assert!(matches!(
            retained_image(&before).prepare_query_creation(
                definition("First", "Answer", 43),
                &mut budget(),
                &|| false,
            ),
            Err(QueryCreationError::Source)
        ));

        let source = raw::source(existing_rows, Hosts::Alternate, "First", "Answer", 43, at());
        assert_eq!(rows(&source.before, 3), existing_rows as u16);
        assert_eq!(rows(&source.before, 7), 1);
        assert_eq!(rows(&source.expected, 3), existing_rows as u16);
        assert_eq!(rows(&source.expected, 7), 5);
        let candidate = retained_image(&source.before)
            .prepare_query_creation(definition("First", "Answer", 43), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(candidate.candidate_bytes(), source.expected);
        assert_eq!(
            candidate.storage_change(),
            NativeQueryStorageChange::Initialized {
                native_table_id: 8,
                definition_page: 8,
            }
        );
    }
}

#[test]
fn maximum_qualified_map_host_reconstructs_fresh_pending_and_committed_originals() {
    let source = raw::source(251, Hosts::Only, "First", "Answer", 43, at());
    for committed in [false, true] {
        let plan = retained_image(&source.before)
            .plan_recoverable_query_creation(
                definition("First", "Answer", 43),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), source.expected);
        let encoded = plan.intent().encode(&mut budget(), &|| false).unwrap();
        // NO_CLONE: Durable publication owns mutable bytes independently of
        // the immutable complete raw BEFORE and AFTER recovery oracles.
        let mut target = MemoryStorage::new(source.before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = plan
            .prepare_retained(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
        }
        let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
        let outcome = NativeQueryCreationIntent::decode(&encoded, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(918), &mut budget(), &|| false)
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
    }
}
