//! Number-specific publication never accepts query-only or stale save authority.
use super::*;
use crate::workspace::{
    ObjectTab, ObjectTarget,
    objects::{MAX_RETAINED_BYTES, session::Session},
};
use crate::{Cell, Data, SavedQuerySqlEligibility, SavedQueryTarget};
use tablecraft_builders::table::FieldAdditionDeclaration;

fn state() -> State {
    let mut state = crate::workspace::objects::tests::state();
    state.field_addition_available = true;
    state.saved_query_sql_edit_available = true;
    state
}

fn data(table: &str, text: &str) -> Data {
    Data {
        source_revision: Some(7),
        row_ordinals: None,
        table: table.into(),
        offset: 0,
        columns: vec!["Value".into()],
        rows: vec![vec![Cell::scalar(text)]],
        has_more: false,
    }
}

fn open_table(state: &mut State, name: &str) -> ObjectTab {
    let tab = state
        .prepare_object_open(crate::workspace::objects::tests::table(name))
        .unwrap()
        .tab;
    assert!(state.set_table_data(data(name, "old cached row")));
    tab
}

fn open_saved(state: &mut State, proposed: Option<String>) -> ObjectTab {
    let target = SavedQueryTarget {
        document_revision: 7,
        object_id: 9,
        name: "Q".into(),
    };
    state.begin_saved_query_inspection(target.clone(), 100);
    assert!(state.set_saved_query_details(100, &target, vec![]));
    let inspection = state.begin_saved_query_sql_inspection(target, 101).unwrap();
    assert!(state.set_saved_query_sql_inspection(
        101,
        &inspection,
        Some(" select 42 AS [Answer];\n".into()),
        SavedQuerySqlEligibility::QualifiedExpressionSelect,
    ));
    if let Some(text) = proposed {
        assert!(state.set_saved_query_sql_text(text));
    }
    state.active_object().unwrap()
}

fn saving(state: &mut State, tab: &ObjectTab) -> FieldAdditionRequest {
    assert!(state.begin_field_addition_inspection(tab, 10).is_ok());
    assert!(state.set_field_addition_availability(tab, 10, true, false, Default::default()));
    assert!(state.begin_field_addition_name(
        tab,
        "Field1".into(),
        FieldAdditionDeclaration::NumberLongZero
    ));
    let request = state.field_addition_review_request().unwrap();
    assert!(state.can_rebind_field_addition_layout(&request));
    assert!(state.begin_field_addition_review(&request, 20));
    assert!(state.accept_field_addition_review(&request, 20, 30));
    assert!(state.bind_field_addition_save(&request, 20, 30, 40));
    assert!(
        state.begin_field_addition_save_status(FieldAdditionSaveIdentity {
            request_id: 20,
            review_id: 30,
            save_id: 40,
        })
    );
    let identity = FieldAdditionSaveIdentity {
        request_id: 20,
        review_id: 30,
        save_id: 40,
    };
    assert!(state.set_field_addition_save_outcome(
        identity,
        crate::EditSaveOutcome::Saved("known AFTER".into())
    ));
    assert!(state.can_acknowledge_field_addition_save(identity));
    // Native ACK/full-witness success are host preconditions, qualified in the
    // runtime/host tests. This state-only scenario represents that handoff.
    assert!(state.begin_field_addition_reopen(identity));
    state.busy = true;
    request
}

fn fresh(state: &State) -> Database {
    let old = state.database.as_ref().unwrap();
    Database {
        document_revision: 8,
        title_path: old.title_path.clone(),
        title: old.title.clone(),
        format: old.format.clone(),
        tables: old
            .tables
            .iter()
            .map(|table| crate::Table {
                name: table.name.clone(),
                local: table.local,
                system: table.system,
                storage: table.storage.clone(),
            })
            .collect(),
        queries: old
            .queries
            .iter()
            .map(|query| crate::SavedQuery {
                object_id: query.object_id,
                name: query.name.clone(),
                system: query.system,
            })
            .collect(),
    }
}

#[test]
fn same_name_schema_reopen_retains_all_tabs_scratch_clean_sql_and_native_preferences() {
    let mut state = state();
    let table_b = state
        .prepare_object_open(crate::workspace::objects::tests::table("B"))
        .unwrap()
        .tab;
    assert!(state.set_object_load_offset(&table_b, 23));
    let mut parked_data = data("B", "old cached row");
    parked_data.offset = 23;
    assert!(state.set_table_data(parked_data));
    let saved = open_saved(&mut state, None);
    let buffers = state
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .rebind_buffer_identity();
    let scratch = state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    assert!(state.replace_query_sql(" select [Value] from [A];\n".into()));
    let target = open_table(&mut state, "A");
    let request = saving(&mut state, &target);
    let old = state.object_tabs();
    let sql_revision = state.query_sql_revision;
    let database = fresh(&state);
    assert!(state.resolve_field_addition_after_reopen(20, &request, 30, 40, database));
    assert!(state.busy);
    assert_eq!(state.active_object().unwrap().id, target.id);
    assert!(state.data.is_none());
    assert!(state.schema.is_none());
    assert_eq!(state.sql, " select [Value] from [A];\n");
    assert_eq!(state.query_sql_revision, sql_revision + 1);
    assert!(state.matches_field_addition_save(
        &request,
        FieldAdditionSaveIdentity {
            request_id: 20,
            review_id: 30,
            save_id: 40
        }
    ));
    for (old, new) in old.iter().zip(state.object_tabs()) {
        assert_eq!(new.id, old.id);
        assert_eq!(new.target.kind, old.target.kind);
        assert_eq!(new.target.document_revision, 8);
    }
    for entry in &state.objects.entries {
        if let Some(session) = &entry.session {
            assert_eq!(entry.retained_bytes, session.retained_bytes());
        }
    }
    assert!(state.clear_field_addition_save(
        &request,
        FieldAdditionSaveIdentity {
            request_id: 20,
            review_id: 30,
            save_id: 40,
        }
    ));
    state.busy = false;
    let saved = state
        .object_tabs()
        .into_iter()
        .find(|tab| tab.id == saved.id)
        .unwrap();
    assert!(state.activate_object(&saved));
    assert_eq!(
        state
            .saved_query
            .sql
            .as_ref()
            .unwrap()
            .rebind_buffer_identity(),
        buffers
    );
    assert_eq!(
        state
            .saved_query
            .sql
            .as_ref()
            .unwrap()
            .target()
            .document_revision,
        8
    );
    let scratch = state
        .object_tabs()
        .into_iter()
        .find(|tab| tab.id == scratch.id)
        .unwrap();
    assert!(state.activate_object(&scratch));
    assert!(state.query_data.is_none());
    let table_b = state
        .object_tabs()
        .into_iter()
        .find(|tab| tab.id == table_b.id)
        .unwrap();
    assert!(state.activate_object(&table_b));
    assert_eq!(state.active_object_load().unwrap().offset, 23);
    assert!(state.data.is_none());
}

#[test]
fn wrong_save_identity_stale_request_or_catalog_refuse_without_state_changes() {
    let mut state = state();
    let target = open_table(&mut state, "A");
    let request = saving(&mut state, &target);
    let old_tabs = state.object_tabs();
    let data_pointer = state.data.as_ref().unwrap().rows.as_ptr();
    for (request_id, review_id, save_id) in [(21, 30, 40), (20, 31, 40), (20, 30, 41)] {
        let database = fresh(&state);
        assert!(!state.resolve_field_addition_after_reopen(
            request_id, &request, review_id, save_id, database
        ));
    }
    let stale_request = FieldAdditionRequest {
        declaration: FieldAdditionDeclaration::NumberLongZero,
        tab: request.tab.clone(),
        draft_revision: request.draft_revision + 1,
        column: request.column.clone(),
    };
    let database = fresh(&state);
    assert!(!state.resolve_field_addition_after_reopen(20, &stale_request, 30, 40, database));
    let mut wrong = fresh(&state);
    wrong.tables[0].name = "Wrong".into();
    assert!(!state.resolve_field_addition_after_reopen(20, &request, 30, 40, wrong));
    let mut stale = fresh(&state);
    stale.document_revision = 7;
    assert!(!state.resolve_field_addition_after_reopen(20, &request, 30, 40, stale));
    assert_eq!(state.object_tabs(), old_tabs);
    assert_eq!(state.database.as_ref().unwrap().document_revision, 7);
    assert_eq!(state.data.as_ref().unwrap().rows.as_ptr(), data_pointer);
    assert!(state.field_addition_request_is_current(&request));
}

#[test]
fn large_obsolete_native_rows_do_not_create_a_half_quota_restriction() {
    let mut state = state();
    let target = open_table(&mut state, "A");
    let mut old_row = String::with_capacity(48 * 1024 * 1024);
    old_row.push_str("old native result");
    state.data.as_mut().unwrap().rows[0][0].text = old_row;
    let request = saving(&mut state, &target);
    assert!(state.object_bytes().unwrap() > MAX_RETAINED_BYTES / 2);
    let database = fresh(&state);
    assert!(state.can_rebind_field_addition_layout(&request));
    assert!(state.resolve_field_addition_after_reopen(20, &request, 30, 40, database));
    assert!(state.data.is_none());
    assert!(state.object_bytes().unwrap() < 1024 * 1024);
}

#[test]
fn fresh_catalog_memory_refusal_leaves_existing_text_and_rows_untouched() {
    let mut state = state();
    let target = open_table(&mut state, "A");
    let mut old_row = String::with_capacity(40 * 1024 * 1024);
    old_row.push_str("retained current row");
    state.data.as_mut().unwrap().rows[0][0].text = old_row;
    let request = saving(&mut state, &target);
    let old_pointer = state.data.as_ref().unwrap().rows[0][0].text.as_ptr();
    let mut database = fresh(&state);
    let mut format = String::with_capacity(40 * 1024 * 1024);
    format.push_str(&database.format);
    database.format = format;
    assert!(!state.resolve_field_addition_after_reopen(20, &request, 30, 40, database));
    assert_eq!(
        state.data.as_ref().unwrap().rows[0][0].text.as_ptr(),
        old_pointer
    );
    assert_eq!(state.database.as_ref().unwrap().document_revision, 7);
    assert!(state.field_addition_request_is_current(&request));
}

#[test]
fn staged_wrong_session_refuses_before_any_clean_sql_buffer_is_taken() {
    let mut state = state();
    let saved = open_saved(&mut state, None);
    let buffers = state
        .saved_query
        .sql
        .as_ref()
        .unwrap()
        .rebind_buffer_identity();
    let target = open_table(&mut state, "A");
    let request = saving(&mut state, &target);
    let database = fresh(&state);
    let mut prepared =
        replacement::prepare(&mut state, &database, replacement::Transfer::KeepAll).unwrap();
    let wrong = prepared
        .objects
        .entries
        .iter_mut()
        .find(|entry| entry.tab.id == saved.id)
        .unwrap();
    wrong.session = Some(Session::empty(&ObjectKind::Sql));
    assert!(!replacement::publish(
        &mut state,
        prepared,
        database,
        super::super::catalog::Change::Unchanged
    ));
    let original = state
        .objects
        .entries
        .iter()
        .find(|entry| entry.tab.id == saved.id)
        .unwrap();
    let Some(Session::Saved { state: owner, .. }) = &original.session else {
        panic!("original clean saved-query owner was discarded");
    };
    assert_eq!(
        owner.sql.as_ref().unwrap().rebind_buffer_identity(),
        buffers
    );
    assert_eq!(state.database.as_ref().unwrap().document_revision, 7);
    assert!(state.field_addition_request_is_current(&request));
}

#[test]
fn an_unrelated_dirty_sql_owner_blocks_schema_publication_without_discarding_text() {
    let mut state = state();
    let saved = open_saved(&mut state, None);
    let target = open_table(&mut state, "A");
    let request = saving(&mut state, &target);
    let mut dirty_source = self::state();
    let mut proposed = String::with_capacity(8192);
    proposed.push_str(" select 99 AS [Answer];\n");
    open_saved(&mut dirty_source, Some(proposed));
    let dirty = std::mem::take(&mut dirty_source.saved_query);
    let dirty_buffers = dirty.sql.as_ref().unwrap().rebind_buffer_identity();
    let entry = state
        .objects
        .entries
        .iter_mut()
        .find(|entry| entry.tab.id == saved.id)
        .unwrap();
    let Some(Session::Saved { state: owner, .. }) = &mut entry.session else {
        panic!("expected parked SQL owner");
    };
    **owner = dirty;
    entry.retained_bytes = entry.session.as_ref().and_then(Session::retained_bytes);
    assert!(!state.can_rebind_field_addition_layout(&request));
    let database = fresh(&state);
    assert!(!state.resolve_field_addition_after_reopen(20, &request, 30, 40, database));
    let entry = state
        .objects
        .entries
        .iter()
        .find(|entry| entry.tab.id == saved.id)
        .unwrap();
    let Some(Session::Saved { state: owner, .. }) = &entry.session else {
        panic!("dirty owner lost");
    };
    assert_eq!(
        owner.sql.as_ref().unwrap().rebind_buffer_identity(),
        dirty_buffers
    );
    assert!(state.has_modified_saved_query_sql());
    assert_eq!(state.database.as_ref().unwrap().document_revision, 7);
}

#[test]
fn an_unsaved_table_editor_blocks_publication_and_keeps_its_field_text() {
    let mut state = state();
    let target = open_table(&mut state, "A");
    let request = saving(&mut state, &target);
    let mut editor = crate::workspace::table_draft::Editor::initial("Unrelated".into());
    editor
        .draft
        .add_named_field(tablecraft_builders::table::DraftFieldType::Number, "KeepMe")
        .unwrap();
    let field_pointer = editor.draft.fields()[1].name().as_ptr();
    state.table_draft = Some(editor);
    assert!(!state.can_rebind_field_addition_layout(&request));
    let database = fresh(&state);
    assert!(!state.resolve_field_addition_after_reopen(20, &request, 30, 40, database));
    let editor = state.table_draft.as_ref().unwrap();
    assert_eq!(editor.draft.fields()[1].name(), "KeepMe");
    assert_eq!(editor.draft.fields()[1].name().as_ptr(), field_pointer);
    assert_eq!(state.database.as_ref().unwrap().document_revision, 7);
}
