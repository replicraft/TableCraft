use super::{field_label, harness, state};
use crate::{Locale, Localizer, Theme, workspace::InsertInput};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::Queryable;
use tablecraft_builders::table::DraftFieldType;

#[test]
fn actual_description_cells_preserve_exact_text_protected_id_and_view_type_changes() {
    let (mut state, field) = state();
    assert!(state.show_table_draft_design());
    let mut harness = harness(state);
    let label = field_label(
        &harness.state().0,
        "table-draft-design-description",
        "Field1",
    );
    harness
        .get_by_role_and_label(Role::MultilineTextInput, &label)
        .focus();
    harness.run_steps(1);
    harness.event(egui::Event::Text("  Ω漢字\nsecond line\t ".into()));
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(
        editor.draft.fields()[1].description(),
        Some("  Ω漢字\nsecond line\t ")
    );
    let revision = editor.draft.revision();
    assert!(editor.property_revision() > 0);
    assert!(
        harness
            .state_mut()
            .0
            .set_table_draft_field_type(field, DraftFieldType::Currency)
    );
    assert!(harness.state_mut().0.show_table_draft_datasheet());
    harness.run_steps(2);
    assert!(harness.state_mut().0.show_table_draft_design());
    harness.run_steps(2);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, &label)
            .value()
            .as_deref(),
        Some("  Ω漢字\nsecond line\t ")
    );
    assert!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .revision()
            > revision
    );

    let label = field_label(&harness.state().0, "table-draft-design-description", "ID");
    harness
        .get_by_role_and_label(Role::MultilineTextInput, &label)
        .focus();
    harness.run_steps(1);
    harness.event(egui::Event::Text("Generated identifier".into()));
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(
        editor.draft.fields()[0].description(),
        Some("Generated identifier")
    );
    assert!(editor.draft.fields()[0].is_primary());
    assert_eq!(
        editor
            .record
            .inputs(&editor.draft.snapshot("Items").unwrap())
            .unwrap()[0],
        InsertInput::Omitted
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn oversized_paste_is_atomic_localized_and_blocks_save_until_actual_correction() {
    for locale in Locale::ALL {
        for theme in [Theme::Light, Theme::Dark] {
            let (mut state, field) = state();
            state.localizer = Localizer::new(locale).unwrap();
            state
                .table_draft
                .as_mut()
                .unwrap()
                .draft
                .set_field_description(field, Some("Kept Ω"))
                .unwrap();
            assert!(state.show_table_draft_design());
            let label = field_label(&state, "table-draft-design-description", "Field1");
            let mut harness = harness(state);
            crate::theme::set_theme(&harness.ctx, theme);
            harness.run_steps(2);
            let original = harness
                .state()
                .0
                .table_draft
                .as_ref()
                .unwrap()
                .draft
                .snapshot("Items")
                .unwrap();
            let property_revision = harness
                .state()
                .0
                .table_draft
                .as_ref()
                .unwrap()
                .property_revision();
            harness
                .get_by_role_and_label(Role::MultilineTextInput, &label)
                .focus();
            harness.run_steps(1);
            harness.event(egui::Event::Paste("😀".repeat(128)));
            harness.run_steps(2);
            assert_eq!(
                harness
                    .get_by_role_and_label(Role::MultilineTextInput, &label)
                    .value()
                    .as_deref(),
                Some("Kept Ω")
            );
            let editor = harness.state().0.table_draft.as_ref().unwrap();
            assert!(editor.draft.snapshot("Items").unwrap() == original);
            assert!(editor.property_revision() > property_revision);
            assert!(!harness.state().0.can_save_table_draft());
            assert!(!harness.state_mut().0.request_table_draft_save());
            let error = harness
                .state()
                .0
                .localizer
                .text("table-draft-description-invalid");
            assert!(harness.query_by_label(&error).is_some());

            harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
            harness.event(egui::Event::Text("Corrected 漢字".into()));
            harness.run_steps(2);
            assert_eq!(
                harness
                    .state()
                    .0
                    .table_draft
                    .as_ref()
                    .unwrap()
                    .draft
                    .fields()[1]
                    .description(),
                Some("Corrected 漢字")
            );
            assert!(harness.state().0.can_save_table_draft());
            assert!(harness.state().1.is_empty());
            assert_eq!(
                harness.state().0.localizer.diagnostics().missing_messages,
                0
            );
            assert_eq!(
                harness.state().0.localizer.diagnostics().english_fallbacks,
                0
            );
        }
    }
}

#[test]
fn raw_description_witness_invalidates_delete_and_discard_without_metadata_change() {
    let (mut state, field) = state();
    let revision = state.table_draft.as_ref().unwrap().draft.revision();
    assert!(state.request_table_draft_field_delete(field));
    assert!(
        state
            .table_draft
            .as_mut()
            .unwrap()
            .properties
            .record_raw_edit()
    );
    assert_eq!(
        state.table_draft.as_ref().unwrap().draft.revision(),
        revision
    );
    assert!(!state.confirm_table_draft_field_delete());
    let tab = state.active_object().unwrap();
    state.table_draft_discard_approved = Some((
        tab.clone(),
        revision,
        state.table_draft_record_revision_for(&tab).unwrap(),
        state.table_draft_property_revision_for(&tab).unwrap(),
    ));
    assert!(
        state
            .table_draft
            .as_mut()
            .unwrap()
            .properties
            .record_raw_edit()
    );
    assert!(!state.close_object(&tab));
    assert!(state.has_unsaved_table_drafts());
    assert!(state.table_draft_discard_approved.is_none());
}
