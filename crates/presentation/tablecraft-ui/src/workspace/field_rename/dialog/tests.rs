//! Bounded source/warning owners must reserve space before native view layout.
use crate::workspace::field_rename::test_support::{accepted, observed};
use crate::{Action, EditSaveOutcome, Locale, Localizer, State};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};

fn presentation(returned: bool, closed: bool) -> State {
    if !returned {
        return observed().0;
    }
    let (mut state, request) = accepted(true);
    let identity = state
        .prepare_field_rename_save(&request, 81, 90)
        .unwrap()
        .bind(100);
    assert!(state.set_field_rename_save_outcome(
        identity,
        EditSaveOutcome::Saved("illustrative outcome".into())
    ));
    assert!(state.begin_field_rename_reopen(identity));
    assert!(state.fail_field_rename_reopen(identity, "illustrative read error".into()));
    assert!(state.return_field_rename_to_draft(&request, identity, None));
    if closed {
        assert!(state.discard_field_rename(&request.target, request.draft_revision));
    }
    state
}

#[test]
fn source_and_retained_warning_controls_fit_normal_and_compact_localized_viewports() {
    for locale in [Locale::EnUs, Locale::De, Locale::Ja] {
        for size in [egui::vec2(1280.0, 820.0), egui::vec2(860.0, 640.0)] {
            for (returned, closed) in [(false, false), (true, false), (true, true)] {
                let mut state = presentation(returned, closed);
                state.localizer = Localizer::new(locale).unwrap();
                let mut harness = Harness::builder().with_size(size).build_ui_state(
                    |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                    (state, Vec::new()),
                );
                crate::configure(&harness.ctx);
                harness.run_steps(3);
                let viewport = harness.ctx.viewport_rect();
                let key = if returned {
                    "field-rename-close-editor"
                } else {
                    "field-rename-cancel"
                };
                if !closed {
                    let label = harness.state().0.localizer.text(key);
                    let button = harness.get_by_role_and_label(Role::Button, &label);
                    assert!(
                        viewport.contains_rect(button.rect()),
                        "{key}: {:?} in {viewport:?}",
                        button.rect()
                    );
                }
                let navigation = harness.state().0.localizer.text("datasheet-page-tools");
                let button = harness.get_by_role_and_label(Role::Button, &navigation);
                assert!(viewport.contains_rect(button.rect()));
                if returned {
                    let label = harness
                        .state()
                        .0
                        .localizer
                        .text("field-rename-warning-saved");
                    let warning = harness.get_by_label(&label);
                    assert!(viewport.contains_rect(warning.rect()));
                    assert!(harness.state().0.field_rename_source_stale());
                    assert!(harness.state().0.native_mutation_blocked());
                }
                assert_eq!(harness.state().0.field_rename_open(), !closed);
                assert!(harness.state().1.is_empty());
            }
        }
    }
}
