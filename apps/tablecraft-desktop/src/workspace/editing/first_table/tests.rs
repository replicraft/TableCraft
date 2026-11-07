use super::test_support::{name_and_confirm, start_save};
use super::*;
use crate::workspace::test_support::{app_harness, finish_reads, finish_save};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use tablecraft_builders::table::DraftFieldType;
use tablecraft_runtime::{SaveFailureKind, SaveOutcome};
use tablecraft_testkit::fixtures::access::{canonical_blank_creation, mutation_fixture};
use tablecraft_ui::{Action, State};

fn created() -> (tempfile::TempDir, PathBuf, Harness<'static, Desktop>) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("New database.accdb");
    let mut harness = app_harness(Desktop::new(egui::Context::default(), None));
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Blank database")
        .click_accesskit();
    harness.run_steps(2);
    harness.state_mut().creation_selected(Some(path.clone()));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Create")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        canonical_blank_creation::source()
    );
    assert!(harness.state().state.has_unsaved_table_drafts());
    (directory, path, harness)
}

fn issue_without_dispatch(harness: &mut Harness<'_, Desktop>) -> TableSaveRequest {
    // Render the exact shared state in a presentation-only host. No Desktop
    // logic or dispatcher can start review/apply before the caller's boundary.
    let state = std::mem::take(&mut harness.state_mut().state);
    let mut presentation = Harness::builder()
        .with_size(egui::vec2(1000.0, 760.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    tablecraft_ui::configure(&presentation.ctx);
    assert!(presentation.state_mut().0.request_table_draft_save());
    presentation.run_steps(2);
    presentation
        .get_by_role_and_label(Role::TextInput, "Table name")
        .focus();
    presentation.run_steps(1);
    presentation
        .get_by_role_and_label(Role::Button, "OK")
        .click_accesskit();
    presentation.run_steps(2);

    let (state, actions) = std::mem::take(presentation.state_mut());
    harness.state_mut().state = state;
    let mut requests = actions.into_iter().filter_map(|action| match action {
        Action::SaveTableDraft { request } => Some(request),
        _ => None,
    });
    let request = requests
        .next()
        .expect("explicit name confirmation issues one request");
    assert!(
        requests.next().is_none(),
        "confirmation must not dispatch twice"
    );
    assert!(harness.state().state.table_draft_save_pending());
    assert!(harness.state().first_table.is_none());
    request
}

#[test]
fn file_new_field_declaration_ctrl_s_and_query_use_the_same_persisted_database() {
    let (_directory, path, mut harness) = created();
    #[cfg(unix)]
    let inode = std::fs::metadata(&path).unwrap().ino();
    assert!(
        harness
            .state_mut()
            .state
            .add_table_draft_field(DraftFieldType::ShortText)
    );
    harness.run_steps(2);
    start_save(&mut harness, "Contacts");
    finish_reads(&mut harness);
    #[cfg(unix)]
    assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().first_table.is_none());
    assert!(!harness.state().state.has_unsaved_table_drafts());
    assert!(!journal_path(&path).exists());
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Contacts");
    assert_eq!(data.columns.len(), 2);
    assert!(data.rows.is_empty());

    harness
        .get_by_role_and_label(Role::Button, "New SQL query")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .focus();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .type_text("SELECT Count(*) AS Total FROM Contacts");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "0"
    );
}

fn insert_generated_row(harness: &mut Harness<'_, Desktop>, companion: Option<&str>) {
    if companion.is_none() {
        // The upper New-row affordance is the actual datasheet control. The
        // separate navigator button shares its accessible name but is chrome.
        let header_bottom = harness
            .get_all_by_role(Role::ColumnHeader)
            .map(|node| node.rect().bottom())
            .fold(0.0, f32::max);
        harness
            .get_all_by_role_and_label(Role::Button, "New row")
            .filter(|node| node.rect().top() >= header_bottom)
            .min_by(|left, right| left.rect().top().total_cmp(&right.rect().top()))
            .expect("the native table exposes a new-record row")
            .focus();
        harness.run_steps(1);
        let zoom = harness.ctx.zoom_factor();
        let modifiers = if harness.ctx.os() == egui::os::OperatingSystem::Mac {
            egui::Modifiers {
                mac_cmd: true,
                command: true,
                ..Default::default()
            }
        } else {
            egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            }
        };
        harness.key_press_modifiers(modifiers, egui::Key::Plus);
        harness.run_steps(1);
        assert_eq!(harness.ctx.zoom_factor(), zoom);
    } else {
        harness
            .get_by_role_and_label(Role::Button, "New")
            .click_accesskit();
    }
    harness.run_steps(1);
    finish_reads(harness);

    // The actual native generation declaration must reach the host. The user
    // cannot enter an ID or turn it into an explicit NULL/value in the draft row.
    assert!(
        harness
            .query_by_role_and_label(Role::MultilineTextInput, "ID")
            .is_none()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "ID")
            .is_none()
    );
    if let Some(value) = companion {
        harness
            .get_by_role_and_label(Role::TextInput, "Field1")
            .focus();
        harness.run_steps(1);
        harness
            .get_by_role_and_label(Role::TextInput, "Field1")
            .type_text(value);
        harness.run_steps(2);
    }
    harness
        .get_by_role_and_label(Role::Button, "Review new row")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(harness.state().editing.insert_review.is_some());
    harness
        .get_by_role_and_label(Role::Button, "Apply new row to original file")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.changed_rows == Some(1) && receipt.schema_change.is_none())
    );
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

#[test]
fn newly_saved_initial_schemas_accept_native_generated_rows_and_refresh_real_contents() {
    for (kind, values) in [
        (None, [None, None]),
        (Some(DraftFieldType::LongInteger), [Some("7"), Some("8")]),
        (
            Some(DraftFieldType::ShortText),
            [Some("Ada"), Some("Grace")],
        ),
    ] {
        let (_directory, path, mut harness) = created();
        #[cfg(unix)]
        let inode = std::fs::metadata(&path).unwrap().ino();
        if let Some(kind) = kind {
            assert!(harness.state_mut().state.add_table_draft_field(kind));
        }
        harness.run_steps(2);
        start_save(&mut harness, "Contacts");
        finish_reads(&mut harness);
        assert!(!harness.state().state.has_unsaved_table_drafts());
        assert!(harness.state().state.data.as_ref().unwrap().rows.is_empty());

        for (position, value) in values.into_iter().enumerate() {
            insert_generated_row(&mut harness, value);
            #[cfg(unix)]
            assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
            assert!(!journal_path(&path).exists());
            assert!(!harness.state().editing.has_save());
            let data = harness.state().state.data.as_ref().unwrap();
            assert_eq!(data.table, "Contacts");
            assert_eq!(data.rows.len(), position + 1);
            assert_eq!(data.rows[position][0].text, (position + 1).to_string());
            if let Some(value) = value {
                assert_eq!(data.rows[position][1].text, value);
            }
        }
    }
}

#[test]
fn cancelling_the_read_only_review_keeps_the_named_draft_and_original_file() {
    let (_directory, path, mut harness) = created();
    assert!(
        harness
            .state_mut()
            .state
            .add_table_draft_field(DraftFieldType::LongInteger)
    );
    harness.run_steps(2);
    let request = issue_without_dispatch(&mut harness);
    harness.state_mut().save_first_table(request);
    let request = harness.state().first_table.as_ref().unwrap().request;
    harness.state_mut().cancel_first_table_save(request);
    harness.run_steps(3);
    assert!(harness.state().first_table.is_none());
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.table_draft_save_pending());
    assert!(harness.state().state.has_modified_table_drafts());
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "Table name")
            .is_some()
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        canonical_blank_creation::source()
    );
    assert!(!journal_path(&path).exists());
}

#[test]
fn external_source_change_retains_the_draft_until_explicit_resource_release() {
    let (_directory, path, mut harness) = created();
    assert!(
        harness
            .state_mut()
            .state
            .add_table_draft_field(DraftFieldType::ShortText)
    );
    harness.run_steps(2);
    let request = issue_without_dispatch(&mut harness);
    harness.state_mut().save_first_table(request);
    let mut changed = std::fs::read(&path).unwrap();
    changed[1024] ^= 1;
    std::fs::write(&path, &changed).unwrap();
    let report = finish_save(&mut harness);
    let SaveOutcome::Failed(failure) = report.outcome else {
        panic!("source change must refuse: {report:?}");
    };
    assert_eq!(failure.kind, SaveFailureKind::SourceChanged);
    assert!(failure.can_release);
    assert!(harness.state().state.has_modified_table_drafts());
    let release = harness.state().state.localizer.text("editing-release");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, &release)
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().first_table.is_none());
    assert!(harness.state().state.has_modified_table_drafts());
    assert_eq!(std::fs::read(&path).unwrap(), changed);
    assert!(!journal_path(&path).exists());
}

#[test]
fn saving_from_the_close_confirmation_resumes_the_original_file_open() {
    let (directory, path, mut harness) = created();
    assert!(
        harness
            .state_mut()
            .state
            .add_table_draft_field(DraftFieldType::LongInteger)
    );
    let other = directory.path().join("Other.accdb");
    let other_bytes = mutation_fixture::with_rows(&[(1, 20)]);
    std::fs::write(&other, &other_bytes).unwrap();
    harness.state_mut().file_selected(Some(other.clone()));
    harness.run_steps(2);
    assert!(harness.state().deferred_draft_action.is_some());
    let saves: Vec<_> = harness
        .get_all_by_role(Role::Button)
        .filter(|node| {
            node.accesskit_node().label().as_deref() == Some("Save")
                && !node.accesskit_node().is_disabled()
        })
        .collect();
    assert_eq!(saves.len(), 1, "only the modal Save may be enabled");
    saves[0].click_accesskit();
    name_and_confirm(&mut harness, "Contacts");
    finish_reads(&mut harness);
    assert!(harness.state().deferred_draft_action.is_none());
    assert!(harness.state().first_table.is_none());
    assert!(!harness.state().state.has_unsaved_table_drafts());
    assert_eq!(
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .title_path
            .as_deref(),
        other.to_str()
    );
    assert_ne!(
        std::fs::read(&path).unwrap(),
        canonical_blank_creation::source()
    );
    assert_eq!(std::fs::read(&other).unwrap(), other_bytes);
}
