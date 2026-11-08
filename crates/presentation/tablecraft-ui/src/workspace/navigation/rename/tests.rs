use super::*;
use crate::workspace::navigation::tests::harness;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;

fn target() -> ObjectTarget {
    ObjectTarget {
        document_revision: 17,
        kind: ObjectKind::Table {
            name: "First".into(),
        },
    }
}

#[test]
fn focused_closed_table_f2_and_enter_issue_exact_names_without_opening_an_object() {
    let mut harness = harness();
    harness.state_mut().0.table_rename_available = true;
    // Keep the actual object label distinct from the record navigator's First button.
    harness.state_mut().0.database.as_mut().unwrap().tables[0].name = "Items 雪😀".into();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Items 雪😀")
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::F2);
    harness.run_steps(2);
    assert!(harness.state().0.table_rename_open());
    harness
        .get_by_role_and_label(Role::TextInput, "New name")
        .focus();
    harness
        .get_by_role_and_label(Role::TextInput, "New name")
        .type_text("雪 New name");
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    let [Action::RenameTable { request }] = harness.state().1.as_slice() else {
        panic!("one exact rename request is expected");
    };
    assert_eq!(request.document_revision, 17);
    assert_eq!(request.table, "Items 雪😀");
    assert_eq!(request.new_name, "雪 New name");
    assert!(harness.state().0.table_rename_request_current(request));
    assert!(harness.state().0.object_tabs().is_empty());
    assert!(harness.state().0.data.is_none());
}

#[test]
fn unsupported_host_search_focus_open_tabs_and_stale_revision_never_begin_rename() {
    let mut harness = harness();
    assert!(!harness.state_mut().0.begin_table_rename(target()));
    harness.state_mut().0.table_rename_available = true;
    harness
        .get_by_role_and_label(Role::TextInput, "Search objects")
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::F2);
    harness.run_steps(2);
    assert!(!harness.state().0.table_rename_open());
    let mut stale = target();
    stale.document_revision += 1;
    assert!(!harness.state_mut().0.begin_table_rename(stale));
    assert!(
        harness
            .state_mut()
            .0
            .prepare_object_open(target())
            .is_some()
    );
    assert!(!harness.state_mut().0.begin_table_rename(target()));
    assert!(harness.state().1.is_empty());
}

#[test]
fn escape_before_review_changes_nothing_and_after_save_only_requests_cancellation() {
    let mut harness = harness();
    harness.state_mut().0.table_rename_available = true;
    assert!(harness.state_mut().0.begin_table_rename(target()));
    harness.run_steps(2);
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().0.table_rename_open());
    assert!(harness.state().1.is_empty());

    assert!(harness.state_mut().0.begin_table_rename(target()));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Rename")
        .click_accesskit();
    harness.run_steps(2);
    let request = match &harness.state().1[0] {
        Action::RenameTable { request } => request.clone(),
        _ => panic!("rename request expected"),
    };
    assert!(harness.state_mut().0.begin_table_rename_save(&request, 91));
    harness.state_mut().1.clear();
    harness.run_steps(2);
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(harness.state().0.table_rename_open());
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::CancelEditSave { save_id: 91 }]
    ));
    assert!(!harness.state_mut().0.acknowledge_table_rename_save(91));
    harness.state_mut().0.set_table_rename_save_outcome(
        91,
        EditSaveOutcome::Retained {
            message: "Retained".into(),
            can_recover: true,
            can_release: false,
        },
    );
    assert!(harness.state().0.can_resolve_table_rename_save(91, true));
    assert!(!harness.state().0.can_resolve_table_rename_save(91, false));
    assert!(!harness.state_mut().0.acknowledge_table_rename_save(91));
}
