//! These are presentation comparison tests, not native eligibility evidence.
//! Actual Qualified observations come from the Desktop real-worker journeys.
use super::state;
use crate::workspace::objects::MAX_RETAINED_BYTES;
use crate::workspace::saved_query::sql::draft::{Draft, EligibilityObservation};
use crate::{SavedQuerySqlEligibility, SavedQuerySqlError, SavedQueryTarget, State};

const AUTHORED: &str = " select  99 AS Answer;\n";
const STORED: &str = "SELECT 99 AS [Answer];";

fn target(revision: u64) -> SavedQueryTarget {
    SavedQueryTarget {
        document_revision: revision,
        object_id: 90,
        name: "Saved 空".into(),
    }
}

fn created() -> State {
    let (mut state, target) = state();
    // Install only the source-neutral presentation produced by Created. This
    // supplies no capability and deliberately remains Unavailable throughout.
    state.saved_query.sql = Some(Draft::created(target, AUTHORED, 14).unwrap());
    state
}

fn observe_unavailable(state: &mut State) {
    let request = state
        .begin_created_query_eligibility_inspection(target(7), 20)
        .unwrap();
    assert!(state.set_saved_query_sql_inspection(
        20,
        &request,
        Some(STORED.into()),
        SavedQuerySqlEligibility::Unavailable,
    ));
}

#[test]
fn created_observation_moves_canonical_storage_without_rewriting_authored_buffers() {
    let mut state = created();
    let identity = state
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .rebind_buffer_identity();
    let design_bytes = state.query_design.retained_bytes();
    let before = state
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .retained_bytes()
        .unwrap();
    let request = state
        .begin_created_query_eligibility_inspection(target(7), 20)
        .unwrap();
    let mut stored = String::with_capacity(512);
    stored.push_str(STORED);
    let stored_pointer = stored.as_ptr();
    let stored_capacity = stored.capacity();
    assert!(state.set_saved_query_sql_inspection(
        20,
        &request,
        Some(stored),
        SavedQuerySqlEligibility::Unavailable,
    ));

    let draft = state.saved_query.sql.as_ref().unwrap();
    assert_eq!(draft.rebind_buffer_identity(), identity);
    assert_eq!(draft.original.as_deref(), Some(AUTHORED));
    assert_eq!(draft.proposed, AUTHORED);
    assert_eq!(draft.revision, 14);
    let EligibilityObservation::CreatedObserved(Some(observed)) = &draft.eligibility_observation
    else {
        panic!("canonical observation must have its own bounded owner");
    };
    assert_eq!(observed.as_ptr(), stored_pointer);
    assert_eq!(draft.retained_bytes(), Some(before + stored_capacity));
    assert_eq!(state.query_design.retained_bytes(), design_bytes);
    assert!(!state.saved_query_sql_needs_created_eligibility_inspection());
    assert!(!state.can_edit_saved_query_sql());
    assert!(!state.can_review_saved_query_sql());
}

#[test]
fn ordinary_rebind_compares_observed_storage_and_detects_real_changes() {
    let mut source = created();
    observe_unavailable(&mut source);
    // Mutate only this source-neutral text owner: no native authority is being
    // invented to enable an editor or publish a persisted outcome.
    assert!(
        source
            .saved_query
            .sql
            .as_mut()
            .unwrap()
            .replace_text("SELECT 101 AS Answer".into())
    );
    let identity = source
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .rebind_buffer_identity();
    let mut rebound = state().0;
    rebound.database.as_mut().unwrap().document_revision = 8;
    rebound.saved_query.target = Some(target(8));
    source
        .saved_query
        .move_sql_rebind_to(&mut rebound.saved_query, 8, None);
    assert!(source.saved_query.sql.is_none());
    assert_eq!(
        rebound
            .saved_query
            .sql
            .as_ref()
            .unwrap()
            .rebind_buffer_identity(),
        identity
    );

    let request = rebound
        .begin_saved_query_sql_inspection(target(8), 21)
        .unwrap();
    assert!(rebound.set_saved_query_sql_inspection(
        21,
        &request,
        Some(STORED.into()),
        SavedQuerySqlEligibility::Unavailable,
    ));
    assert!(rebound.saved_query_sql_error().is_none());
    assert_eq!(rebound.saved_query_sql().unwrap().0, AUTHORED);
    assert_eq!(rebound.saved_query_sql().unwrap().1, "SELECT 101 AS Answer");
    assert_eq!(rebound.saved_query_sql().unwrap().2, 15);

    let request = rebound
        .begin_saved_query_sql_inspection(target(8), 22)
        .unwrap();
    assert!(!rebound.set_saved_query_sql_inspection(
        22,
        &request,
        Some("SELECT 100 AS [Answer];".into()),
        SavedQuerySqlEligibility::Unavailable,
    ));
    assert_eq!(
        rebound.saved_query_sql_error(),
        Some(&SavedQuerySqlError::SourceChanged)
    );
    assert_eq!(rebound.saved_query_sql().unwrap().1, "SELECT 101 AS Answer");
    assert_eq!(rebound.saved_query_sql().unwrap().2, 15);
}

#[test]
fn stale_and_failed_created_inspections_never_grant_or_replace_an_observation() {
    let mut state = created();
    let request = state
        .begin_created_query_eligibility_inspection(target(7), 20)
        .unwrap();
    assert!(!state.set_saved_query_sql_inspection(
        19,
        &request,
        Some(STORED.into()),
        SavedQuerySqlEligibility::Unavailable,
    ));
    assert!(state.saved_query_sql_pending());
    assert!(state.fail_saved_query_sql(20, &request, "actual inspection refused".into()));
    assert!(state.saved_query_sql_needs_created_eligibility_inspection());
    assert_eq!(state.saved_query_sql().unwrap().0, AUTHORED);
    assert_eq!(state.saved_query_sql().unwrap().2, 14);
    let pending = state
        .begin_created_query_eligibility_inspection(target(7), 21)
        .unwrap();
    assert!(
        state
            .saved_query
            .sql
            .as_mut()
            .unwrap()
            .replace_text("SELECT 101 AS Answer".into())
    );
    assert!(!state.set_saved_query_sql_inspection(
        21,
        &pending,
        Some(STORED.into()),
        SavedQuerySqlEligibility::Unavailable,
    ));
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 101 AS Answer");
    assert_eq!(state.saved_query_sql().unwrap().2, 15);
    assert!(state.saved_query_sql_needs_created_eligibility_inspection());
}

#[test]
fn ordinary_dirty_drafts_do_not_acquire_created_baseline_exemption() {
    let (mut state, saved_target) = state();
    let request = state
        .begin_saved_query_sql_inspection(saved_target, 10)
        .unwrap();
    assert!(state.set_saved_query_sql_inspection(
        10,
        &request,
        Some("SELECT 42 AS Answer".into()),
        SavedQuerySqlEligibility::Unavailable,
    ));
    assert!(
        state
            .saved_query
            .sql
            .as_mut()
            .unwrap()
            .replace_text("SELECT 43 AS Answer".into())
    );
    assert!(
        state
            .begin_created_query_eligibility_inspection(target(7), 11)
            .is_none()
    );
    let request = state
        .begin_saved_query_sql_inspection(target(7), 12)
        .unwrap();
    assert!(!state.set_saved_query_sql_inspection(
        12,
        &request,
        Some("SELECT 99 AS Answer".into()),
        SavedQuerySqlEligibility::Unavailable,
    ));
    assert_eq!(
        state.saved_query_sql_error(),
        Some(&SavedQuerySqlError::SourceChanged)
    );
    assert_eq!(state.saved_query_sql().unwrap().0, "SELECT 42 AS Answer");
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 43 AS Answer");
}

#[test]
fn successful_saved_baseline_resolution_requires_a_new_native_observation() {
    let mut state = created();
    observe_unavailable(&mut state);
    let draft = state.saved_query.sql.as_mut().unwrap();
    // Comparison-owner control only. Real TCQE publication is covered by the
    // Desktop journey; this exercises the shared post-resolution transition.
    draft.original = Some("SELECT 101 AS Answer".into());
    draft.proposed = "SELECT 101 AS Answer".into();
    draft.saved_baseline_replaced();
    assert!(state.saved_query_sql_needs_created_eligibility_inspection());
    let request = state
        .begin_created_query_eligibility_inspection(target(7), 21)
        .unwrap();
    assert!(state.set_saved_query_sql_inspection(
        21,
        &request,
        Some("SELECT 101 AS [Answer];".into()),
        SavedQuerySqlEligibility::Unavailable,
    ));
    assert_eq!(state.saved_query_sql().unwrap().0, "SELECT 101 AS Answer");
    assert_eq!(state.saved_query_sql().unwrap().2, 14);
    let request = state
        .begin_saved_query_sql_inspection(target(7), 22)
        .unwrap();
    assert!(state.set_saved_query_sql_inspection(
        22,
        &request,
        Some("SELECT 101 AS [Answer];".into()),
        SavedQuerySqlEligibility::Unavailable,
    ));
    assert!(state.saved_query_sql_error().is_none());
}

#[test]
fn identical_storage_observation_reuses_the_existing_original_owner() {
    let mut state = created();
    let before = state.saved_query.sql.as_ref().unwrap().retained_bytes();
    let request = state
        .begin_created_query_eligibility_inspection(target(7), 20)
        .unwrap();
    assert!(state.set_saved_query_sql_inspection(
        20,
        &request,
        Some(AUTHORED.into()),
        SavedQuerySqlEligibility::Unavailable,
    ));
    let draft = state.saved_query.sql.as_ref().unwrap();
    assert_eq!(draft.retained_bytes(), before);
    assert!(matches!(
        draft.eligibility_observation,
        EligibilityObservation::CreatedObservedOriginal
    ));
    let request = state
        .begin_saved_query_sql_inspection(target(7), 21)
        .unwrap();
    assert!(!state.set_saved_query_sql_inspection(
        21,
        &request,
        Some(STORED.into()),
        SavedQuerySqlEligibility::Unavailable,
    ));
    assert_eq!(
        state.saved_query_sql_error(),
        Some(&SavedQuerySqlError::SourceChanged)
    );
    assert_eq!(state.saved_query_sql().unwrap().0, AUTHORED);
}

#[test]
fn refused_canonical_retention_leaves_the_exact_pending_draft_unchanged() {
    let mut state = created();
    let request = state
        .begin_created_query_eligibility_inspection(target(7), 20)
        .unwrap();
    let before = state.saved_query.sql.as_ref().unwrap().retained_bytes();
    let mut oversized = String::with_capacity(MAX_RETAINED_BYTES);
    oversized.push_str(STORED);
    assert!(!state.set_saved_query_sql_inspection(
        20,
        &request,
        Some(oversized),
        SavedQuerySqlEligibility::Unavailable,
    ));
    assert_eq!(
        state.saved_query.sql.as_ref().unwrap().retained_bytes(),
        before
    );
    assert!(state.saved_query_sql_pending());
    assert!(state.saved_query_sql_needs_created_eligibility_inspection());
    assert_eq!(state.saved_query_sql().unwrap().0, AUTHORED);
    assert_eq!(state.saved_query_sql().unwrap().1, AUTHORED);
    assert_eq!(state.saved_query_sql().unwrap().2, 14);
    assert!(state.set_saved_query_sql_inspection(
        20,
        &request,
        Some(STORED.into()),
        SavedQuerySqlEligibility::Unavailable,
    ));
}
