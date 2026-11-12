//! A real SQL save renews authority while preserving workspace tab preferences.
use super::*;
use tablecraft_testkit::fixtures::access::saved_queries::writable;

const UPDATE: &str = "UPDATE Items SET Score=Score+7 WHERE Id>=2";

fn run_sql(harness: &mut Harness<'_, Desktop>, sql: &str) {
    assert!(harness.state_mut().state.replace_query_sql(sql.into()));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

fn wait_for_action_review(
    harness: &mut Harness<'_, Desktop>,
    path: &std::path::Path,
    before: &[u8],
) {
    let diagnostic = harness.state().state.localizer.text("diagnostics-copy");
    let deadline = Instant::now() + Duration::from_secs(10);

    loop {
        if harness
            .query_by_role_and_label(Role::Button, "Yes")
            .is_some()
        {
            return;
        }
        if harness
            .query_by_role_and_label(Role::Button, &diagnostic)
            .is_some()
        {
            assert_eq!(std::fs::read(path).unwrap(), before);
            assert!(!harness.state().editing.has_save());
            harness
                .get_by_role_and_label(Role::Button, &diagnostic)
                .click_accesskit();
            harness.run_steps(1);
            let detail = harness
                .output()
                .platform_output
                .commands
                .iter()
                .find_map(|command| {
                    if let egui::OutputCommand::CopyText(text) = command {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .expect("terminal action refusal supplies its real diagnostic");
            panic!("existing saved-query fixture did not qualify for native UPDATE: {detail}");
        }
        assert!(
            Instant::now() < deadline,
            "SQL action did not finish review or refusal"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
}

fn assert_saved_input(harness: &mut Harness<'_, Desktop>, prompt: &str, value: &str) {
    assert!(harness.state().state.selected_cell_text().is_none());
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, prompt)
            .value()
            .as_deref(),
        Some(value)
    );
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().state.selected_cell_text().is_none());
}

#[test]
fn sql_save_ack_keeps_tab_ids_design_and_saved_inputs_but_revokes_old_sessions() {
    let source = writable::source();
    let expected = source.expected;
    let (_directory, path, original, mut harness) = opened(source.before);
    let items = table(harness.state(), "Items");
    // NO_CLONE: The command consumes its target; the test retains the original
    // revision-bearing target for stale replay after the real save.
    harness.state_mut().open_object(items.clone());
    finish_reads(&mut harness);
    let items_tab = tab(harness.state(), &items);
    let old_row = EditTarget {
        document_revision: items.document_revision,
        table: "Items".into(),
        column: "Score".into(),
        parent_ordinal: 1,
    };
    assert!(harness.state().state.can_edit_cell(&old_row));
    harness.state_mut().view_object_schema("Items".into());
    finish_reads(&mut harness);
    let field_name = harness.state().state.localizer.text("design-field-name");
    harness.get_by_label(&field_name);
    assert_eq!(
        harness.state().state.schema.as_ref().unwrap().table,
        "Items"
    );

    let first = saved(harness.state(), "Saved");
    // NO_CLONE: Opening consumes the target while the test retains its saved
    // object identity to locate the fresh tab after acknowledgement.
    harness.state_mut().open_object(first.clone());
    finish_reads(&mut harness);
    run_saved(&mut harness, "First minimum", "2");
    assert_eq!(harness.state().state.selected_cell_text(), Some("2"));
    let first_tab = tab(harness.state(), &first);
    // NO_CLONE: Keep the old native binding independently of the changing UI
    // session so the fresh document must reject that exact stale target.
    let old_saved_target = harness
        .state()
        .state
        .selected_saved_target()
        .unwrap()
        .clone();

    let second = saved(harness.state(), "Empty");
    // NO_CLONE: The consumed command and later fresh-tab lookup need separate
    // owners of the saved object identity.
    harness.state_mut().open_object(second.clone());
    finish_reads(&mut harness);
    run_saved(&mut harness, "Second maximum", "1");
    assert_eq!(harness.state().state.selected_cell_text(), Some("1"));
    let second_tab = tab(harness.state(), &second);

    let scratch = ObjectTarget {
        document_revision: items.document_revision,
        kind: ObjectKind::Sql,
    };
    harness.state_mut().open_object(scratch);
    let sql_tab = harness.state().state.active_object().unwrap();
    run_sql(&mut harness, "SELECT 99 AS Previous");
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "99"
    );
    let before_tabs = harness.state().state.object_tabs();
    assert_eq!(before_tabs.len(), 4);

    run_sql(&mut harness, UPDATE);
    wait_for_action_review(&mut harness, &path, &original);
    assert!(harness.state().state.query_action_dialog_open());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.matched_rows == Some(2) && receipt.changed_rows == Some(2)));
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);

    assert!(!harness.state().editing.has_save());
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    assert_ne!(revision, items.document_revision);
    let fresh_tabs = harness.state().state.object_tabs();
    assert_eq!(fresh_tabs.len(), before_tabs.len());
    for old in &before_tabs {
        let fresh = fresh_tabs
            .iter()
            .find(|tab| tab.id == old.id)
            .expect("same tab ID");
        assert_eq!(fresh.target.kind, old.target.kind);
        assert_eq!(fresh.target.document_revision, revision);
        assert!(!harness.state().state.can_activate_object(old));
    }
    let fresh_sql = fresh_tabs.iter().find(|tab| tab.id == sql_tab.id).unwrap();
    assert_eq!(
        harness.state().state.active_object().as_ref(),
        Some(fresh_sql)
    );
    assert_eq!(harness.state().state.sql, UPDATE);
    assert!(
        harness.state().state.query_data.is_none(),
        "ACK must not replay SQL"
    );
    assert!(harness.state().filter.is_none());
    assert!(harness.state().objects.filters.is_empty());
    assert!(!harness.state().state.can_edit_cell(&old_row));
    assert!(
        !harness
            .state()
            .state
            .can_inspect_saved_query(&old_saved_target)
    );

    let generation = harness.state().generation;
    harness.state_mut().activate_object(first_tab);
    harness.state_mut().close_object(second_tab);
    harness.state_mut().open_object(items);
    assert_eq!(harness.state().generation, generation);
    assert_eq!(
        harness.state().state.active_object().as_ref(),
        Some(fresh_sql)
    );

    let first = fresh_tabs
        .iter()
        .find(|tab| tab.target.kind == first.kind)
        .unwrap();
    // NO_CLONE: Activation consumes a tab; the complete fresh-tab snapshot is
    // retained for the remaining independent identity and preference checks.
    harness.state_mut().activate_object(first.clone());
    finish_reads(&mut harness);
    assert_saved_input(&mut harness, "First minimum", "2");
    let second = fresh_tabs
        .iter()
        .find(|tab| tab.target.kind == second.kind)
        .unwrap();
    // NO_CLONE: The consuming activation must not consume the fresh-tab
    // snapshot still needed to check the restored table tab.
    harness.state_mut().activate_object(second.clone());
    finish_reads(&mut harness);
    assert_saved_input(&mut harness, "Second maximum", "1");

    let fresh_items = fresh_tabs
        .iter()
        .find(|tab| tab.id == items_tab.id)
        .unwrap();
    // NO_CLONE: The vector owns the tab alongside the earlier fresh SQL borrow;
    // activation needs its own small revision-bearing presentation target.
    harness.state_mut().activate_object(fresh_items.clone());
    finish_reads(&mut harness);
    assert!(harness.state().state.data.is_none());
    assert_eq!(
        harness.state().state.schema.as_ref().unwrap().table,
        "Items"
    );

    assert_eq!(std::fs::read(path).unwrap(), expected);
}

#[test]
fn row_save_ack_keeps_parked_scratch_design_draft_without_old_results_or_bindings() {
    let (_directory, path, _original, mut harness) = opened(mutation_fixture::database());
    let items = table(harness.state(), "Items");
    let items_tab = tab(harness.state(), &items);
    harness.state_mut().open_object(ObjectTarget {
        document_revision: items.document_revision,
        kind: ObjectKind::Sql,
    });
    let sql_tab = harness.state().state.active_object().unwrap();
    let sql = "SELECT Id, Score FROM Items WHERE Id>=2 ORDER BY Id DESC";
    run_sql(&mut harness, sql);
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Design View")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    // NO_CLONE: Capture an independent expected draft before the UI session is
    // parked and destroyed by the real save/reopen handoff.
    let draft = harness.state().state.query_design_draft().unwrap().clone();
    // NO_CLONE: Compare the designer's actual canonical emission across save;
    // the live SQL string remains owned and replaceable by the UI.
    let canonical_sql = harness.state().state.sql.clone();
    let old_request = harness.state().state.query_design_request().unwrap();
    assert!(!draft.columns.is_empty());

    harness.state_mut().activate_object(items_tab);
    finish_reads(&mut harness);
    let old_row = EditTarget {
        document_revision: items.document_revision,
        table: "Items".into(),
        column: "Score".into(),
        parent_ordinal: 0,
    };
    // NO_CLONE: The edit command consumes its target; retain the exact old
    // source coordinates to prove the fresh document rejects stale edit rights.
    harness.state_mut().begin_edit(old_row.clone());
    finish_reads(&mut harness);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
        .type_text("42");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Review change")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    harness
        .get_by_role_and_label(Role::Button, "Apply to original file")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.changed_rows == Some(1)));
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);

    let fresh_sql = harness
        .state()
        .state
        .object_tabs()
        .into_iter()
        .find(|tab| tab.id == sql_tab.id)
        .expect("parked scratch tab retains its identity");
    assert_ne!(
        fresh_sql.target.document_revision,
        sql_tab.target.document_revision
    );
    assert!(!harness.state().state.can_activate_object(&sql_tab));
    assert!(!harness.state().state.can_edit_cell(&old_row));
    let fresh_sql_id = fresh_sql.id;
    harness.state_mut().activate_object(fresh_sql);
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.active_object().unwrap().id,
        fresh_sql_id
    );
    assert_eq!(harness.state().state.sql, canonical_sql);
    assert_eq!(harness.state().state.query_design_draft(), Some(&draft));
    assert!(harness.state().state.query_data.is_none());
    assert!(harness.state().state.query_design_request_id().is_none());
    assert!(
        !harness
            .state()
            .state
            .can_import_query_design(&old_request, &canonical_sql)
    );
    assert_eq!(
        std::fs::read(path).unwrap(),
        mutation_fixture::with_rows(&[(1, 42), (2, 20), (3, 30)])
    );
}
