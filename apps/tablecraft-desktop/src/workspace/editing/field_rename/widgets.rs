//! Header/Save widgets compose checked replacement with existing record editing.
use super::tests::finish_schema;
use crate::workspace::Desktop;
use crate::workspace::test_support::{self, finish_reads, inserting::insert_omitted_fields};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::time::{Duration, Instant};
use tablecraft_runtime::{Error, journal_path};
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::{self, after_off::SourceCase},
    canonical_database_options, saved_field_rename,
};
use tablecraft_ui::{ObjectKind, ObjectTarget};

#[test]
fn actual_header_rename_and_save_preserve_tabs_and_scratch_then_omitted_default_uses_new_name() {
    for (case, original_name, replacement) in [
        (SourceCase::Number, "Score", "Total"),
        (SourceCase::Mixed, "Label", "Title"),
    ] {
        let [before, after] = saved_field_rename::pair(case);
        let (_directory, path, _, mut harness) = test_support::opened(before);
        let table = harness.state().state.active_object().unwrap();
        harness
            .state_mut()
            .state
            .prepare_object_open(ObjectTarget {
                document_revision: table.target.document_revision,
                kind: ObjectKind::Sql,
            })
            .unwrap();
        let exact_scratch = "  SELECT [Score], [Label] FROM [Table1]; -- preserve raw text\n";
        assert!(
            harness
                .state_mut()
                .state
                .replace_query_sql(exact_scratch.into())
        );
        harness.state_mut().activate_object(table.clone());
        finish_reads(&mut harness);
        let old_tabs = harness.state().state.object_tabs();
        // Initial staging explicitly supplies only the host support fact. The
        // actual header action must still inspect and review real native bytes.
        harness.state_mut().state.field_rename_available = true;
        harness.run_steps(2);
        propose_name(&mut harness, original_name, replacement);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            saved_field_rename::pair(case)[0],
            "typing is inert"
        );
        harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
        harness.run_steps(1);
        finish_schema(&mut harness);
        assert_eq!(std::fs::read(&path).unwrap(), after);
        assert!(!journal_path(&path).exists());
        assert!(!harness.state().state.field_rename_open());
        let new_tabs = harness.state().state.object_tabs();
        assert_eq!(old_tabs.len(), new_tabs.len());
        for (old, new) in old_tabs.iter().zip(&new_tabs) {
            assert_eq!(old.id, new.id);
            assert_eq!(old.target.kind, new.target.kind);
        }
        assert_eq!(harness.state().state.active_object().unwrap().id, table.id);
        let scratch = new_tabs
            .iter()
            .find(|tab| matches!(tab.target.kind, ObjectKind::Sql))
            .unwrap();
        harness.state_mut().activate_object(scratch.clone());
        finish_reads(&mut harness);
        assert_eq!(harness.state().state.sql, exact_scratch);
        let table = harness
            .state()
            .state
            .object_tabs()
            .into_iter()
            .find(|tab| tab.id == table.id)
            .unwrap();
        harness.state_mut().activate_object(table.clone());
        finish_reads(&mut harness);
        if matches!(case, SourceCase::Number) {
            insert_omitted_fields(
                &mut harness,
                &path,
                &after,
                table.target.document_revision,
                &["Total"],
            );
        }
    }
}

fn propose_name(harness: &mut Harness<'_, Desktop>, original_name: &str, replacement: &str) {
    harness.get_by_label(original_name).click();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run_steps(2);
    let command = harness.get_by_role_and_label(Role::Button, "Rename Field");
    assert!(!command.accesskit_node().is_disabled());
    command.click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    let input = harness.get_by_role_and_label(Role::TextInput, "Field name");
    assert!(!input.accesskit_node().is_read_only());
    input.focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .type_text(replacement);
    harness.run_steps(1);
}

#[test]
fn actual_duplicate_proposal_reaches_native_review_and_retains_exact_name_without_writing() {
    // The standard Mixed pin has no Score. Use the existing raw owners for the
    // independently qualified inline Number + Label + explicit OFF profile.
    let fields = mixed::append_text(
        &mixed::source(mixed::SourceCase::InlineNumber),
        "Label",
        255,
        true,
    );
    let before = canonical_database_options::append_explicit_off_to_owned_page(&fields);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    harness.state_mut().state.field_rename_available = true;
    harness.run_steps(2);
    propose_name(&mut harness, "Label", "Score");
    assert!(
        harness.state().state.can_review_field_rename(),
        "same-width duplicate is not a shape-only UI refusal"
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    assert!(
        harness.state().editing.field_rename.is_some(),
        "actual Review action must start the real native service"
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    let (generation, result) = loop {
        if let Some(reply) = harness.state().worker.poll() {
            break reply;
        }
        assert!(Instant::now() < deadline, "duplicate review did not finish");
        std::thread::yield_now();
    };
    let Err(error) = result else {
        panic!("duplicate review must issue no consuming capability");
    };
    assert!(matches!(error, Error::FieldRenameIntent(_)), "{error}");
    assert_eq!(
        error.to_string(),
        "field rename conflicts with an existing field"
    );
    harness
        .state_mut()
        .field_rename_review_failed(generation, error.to_string());
    assert_eq!(
        harness.state().state.field_rename_proposal().unwrap().1,
        "Score"
    );
    assert!(harness.state().state.error.is_some());
    assert!(harness.state().editing.field_rename.is_none());
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().editing.worker.is_busy());
    assert!(harness.state().state.field_rename_save_status().is_none());
    assert!(!journal_path(&path).exists());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
