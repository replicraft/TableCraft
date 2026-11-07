use super::{AVAILABLE, UNAVAILABLE};
use crate::workspace::{
    Action, State,
    preview::{Preview, Scene},
};
use crate::{Locale, Localizer, Theme};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::table::DraftFieldType;

fn harness(locale: Locale, theme: Theme) -> Harness<'static, (State, Vec<Action>)> {
    let mut preview = Preview::new(Scene::InitialTableDraft);
    let mut state = std::mem::take(preview.state_mut());
    state.localizer = Localizer::new(locale).unwrap();
    state.theme = theme;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
}

fn open_gallery(harness: &mut Harness<'_, (State, Vec<Action>)>) {
    let label = harness.state().0.localizer.text("datasheet-click-to-add");
    harness
        .get_by_role_and_label(Role::Button, &label)
        .click_accesskit();
    harness.run_steps(2);
}

#[test]
fn six_working_choices_add_exact_declared_types_without_native_actions() {
    for (kind, key) in AVAILABLE {
        let mut harness = harness(Locale::EnUs, Theme::Light);
        open_gallery(&mut harness);
        let label = harness.state().0.localizer.text(key);
        harness
            .get_by_role_and_label(Role::Button, &label)
            .click_accesskit();
        harness.run_steps(2);
        let state = &harness.state().0;
        let editor = state.table_draft.as_ref().unwrap();
        assert_eq!(editor.draft.fields().len(), 2, "{key}");
        assert_eq!(editor.draft.fields()[1].kind(), kind, "{key}");
        assert!(editor.rename.is_some());
        assert!(state.data.is_none());
        assert!(state.database.as_ref().unwrap().tables.is_empty());
        assert!(state.table_selection.position().is_none());
        assert!(harness.state().1.is_empty());
    }
}

#[test]
fn unsupported_choices_are_visible_disabled_and_cannot_mutate_a_draft() {
    let mut harness = harness(Locale::EnUs, Theme::Dark);
    open_gallery(&mut harness);
    for key in UNAVAILABLE {
        let label = harness.state().0.localizer.text(key);
        let button = harness.get_by_role_and_label(Role::Button, &label);
        assert!(button.accesskit_node().is_disabled(), "{key}");
        assert!(
            !button
                .accesskit_node()
                .data()
                .supports_action(egui::accesskit::Action::Click)
        );
        button.click_accesskit();
        harness.run_steps(2);
        assert_eq!(harness.state().0.table_draft_revision(), Some(0));
    }
    assert!(harness.state().1.is_empty());
}

#[test]
fn compact_gallery_keeps_all_translated_choices_and_single_header_trigger() {
    for locale in Locale::ALL {
        for theme in [Theme::Light, Theme::Dark] {
            let mut harness = harness(locale, theme);
            open_gallery(&mut harness);
            for (_, key) in AVAILABLE {
                let label = harness.state().0.localizer.text(key);
                assert!(
                    !harness
                        .get_by_role_and_label(Role::Button, &label)
                        .accesskit_node()
                        .is_disabled(),
                    "{locale:?}/{theme:?}/{key}"
                );
            }
            for key in UNAVAILABLE {
                let label = harness.state().0.localizer.text(key);
                assert!(
                    harness
                        .get_by_role_and_label(Role::Button, &label)
                        .accesskit_node()
                        .is_disabled(),
                    "{locale:?}/{theme:?}/{key}"
                );
            }
            let add = harness.state().0.localizer.text("datasheet-click-to-add");
            assert_eq!(
                harness
                    .get_all_by_role_and_label(Role::Button, &add)
                    .count(),
                1
            );
            assert_eq!(
                harness.state().0.localizer.diagnostics().missing_messages,
                0
            );
        }
    }
}

#[test]
fn header_focus_reveals_click_to_add_after_wide_schema_and_keyboard_adds() {
    let mut harness = harness(Locale::EnUs, Theme::Light);
    for _ in 0..6 {
        assert!(
            harness
                .state_mut()
                .0
                .add_table_draft_field(DraftFieldType::ShortText)
        );
    }
    assert!(
        harness
            .state_mut()
            .0
            .table_draft
            .as_mut()
            .unwrap()
            .commit_rename()
    );
    harness.run_steps(4);
    let add = harness.get_by_role_and_label(Role::Button, "Click to Add");
    assert!(add.is_focused());
    assert!(
        add.rect().left() >= 0.0 && add.rect().right() <= 900.0,
        "{:?}",
        add.rect()
    );
    add.click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Currency")
        .focus();
    harness.run_steps(1);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(
        editor.draft.fields().last().unwrap().kind(),
        DraftFieldType::Currency
    );
    assert_eq!(editor.draft.fields().len(), 8);
    assert!(harness.state().1.is_empty());
}

#[test]
fn german_header_keeps_full_accessible_label_inside_one_fixed_width_column() {
    let harness = harness(Locale::De, Theme::Dark);
    let label = harness.state().0.localizer.text("datasheet-click-to-add");
    let header = harness.get_by_role_and_label(Role::Button, &label);
    assert!(header.rect().width() <= 150.0, "{:?}", header.rect());
    assert!(header.rect().height() <= 22.0, "{:?}", header.rect());
    assert_eq!(
        harness
            .get_all_by_role_and_label(Role::Button, &label)
            .count(),
        1
    );
}
