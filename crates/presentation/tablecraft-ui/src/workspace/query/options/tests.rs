use super::*;
use crate::workspace::query::tests::{harness, result, state};
use crate::{Action, Locale, Localizer, Theme};
use egui::Key;
use egui_kittest::kittest::{NodeT, Queryable};

fn limits() -> QueryRunLimits {
    QueryRunLimits {
        snapshot_rows: 16_384,
        batch_rows: 256,
        native: QueryPoolLimits {
            bytes: 536_870_912,
            rows: 100_000,
            steps: 5_000_000,
        },
        query: QueryPoolLimits {
            bytes: 16_777_216,
            rows: 10_000,
            steps: 1_000_000,
        },
        comparison: QueryPoolLimits {
            bytes: 16_777_216,
            rows: 0,
            steps: 1_000_000,
        },
        presentation: QueryPoolLimits {
            bytes: 8_388_608,
            rows: 0,
            steps: 100_000,
        },
    }
}

#[test]
fn apply_requires_host_validation_and_keeps_draft_sql_and_previous_results_on_rejection() {
    let mut state = state();
    state.sql = "SELECT 99".into();
    assert!(state.set_query_data(Some("SELECT 99".into()), result("99")));
    state.set_query_run_limits(limits());
    assert!(state.open_query_run_limits());
    state.query_limits.draft.as_mut().unwrap().snapshot_rows = 200_000;
    let previous = state.query_data.as_ref().unwrap().rows.as_ptr();
    let mut harness = harness(state);
    harness.run();
    harness.get_by_label("Use for next runs").click();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::ConfigureQueryRunLimits { limits }] if limits.snapshot_rows == 200_000)
    );
    assert_eq!(harness.state().0.query_run_limits(), Some(limits()));
    assert!(harness.state().0.query_run_limits_open());
    assert_eq!(
        harness.state().0.query_data.as_ref().unwrap().rows.as_ptr(),
        previous
    );
    assert_eq!(harness.state().0.sql, "SELECT 99");

    harness
        .state_mut()
        .0
        .reject_query_run_limits("Example host validation refusal".into());
    harness.state_mut().1.clear();
    harness.run();
    assert!(
        harness
            .query_by_label("Example host validation refusal")
            .is_some()
    );
    assert_eq!(
        harness.state().0.query_limits.draft.unwrap().snapshot_rows,
        200_000
    );
    harness.key_press(Key::F5);
    harness.run();
    assert!(
        harness.state().1.is_empty(),
        "preferences must not dispatch a query"
    );

    let accepted = harness.state().0.query_limits.draft.unwrap();
    harness.state_mut().0.set_query_run_limits(accepted);
    assert!(!harness.state().0.query_run_limits_open());
    assert_eq!(
        harness.state().0.query_run_limits().unwrap().snapshot_rows,
        200_000
    );
    assert_eq!(
        harness.state().0.query_data.as_ref().unwrap().rows.as_ptr(),
        previous
    );
}

#[test]
fn escape_discards_only_preferences_and_busy_work_cannot_open_them() {
    let mut state = state();
    state.set_query_run_limits(limits());
    state.busy = true;
    assert!(!state.open_query_run_limits());
    state.busy = false;
    assert!(state.open_query_run_limits());
    assert!(state.edit_dialog_open());
    assert!(
        state.can_change_query_run_limits(),
        "own dialog allows host acknowledgement"
    );
    state.query_limits.draft.as_mut().unwrap().query.steps = 20;
    let mut harness = harness(state);
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().0.query_run_limits_open());
    assert_eq!(harness.state().0.query_run_limits(), Some(limits()));
    assert!(harness.state().1.is_empty());
}

#[test]
fn compact_localized_preferences_keep_decisions_in_the_viewport() {
    for locale in [Locale::EnUs, Locale::De, Locale::Ru, Locale::Ja] {
        for theme in [Theme::Light, Theme::Dark] {
            let mut state = state();
            state.localizer = Localizer::new(locale).unwrap();
            state.theme = theme;
            state.set_query_run_limits(limits());
            assert!(state.open_query_run_limits());
            let apply = state.localizer.text("query-limits-apply");
            let cancel = state.localizer.text("query-limits-cancel");
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::vec2(640.0, 480.0))
                .build_ui_state(
                    |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                    (state, Vec::new()),
                );
            harness.run_steps(3);
            harness.run();
            let viewport = harness.ctx.viewport_rect();
            assert!(
                viewport.contains_rect(harness.get_by_label(&apply).rect()),
                "{locale:?}/{theme:?}"
            );
            assert!(
                viewport.contains_rect(harness.get_by_label(&cancel).rect()),
                "{locale:?}/{theme:?}"
            );
            assert!(harness.state().1.is_empty());
        }
    }
}
