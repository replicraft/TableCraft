//! Ordinary readable SQL with no native writer qualification or capability.
use super::{INSPECTION_ID, LAYOUT_SETTLE_STEPS, SQL_INSPECTION_ID, assert_painted};
use egui::{Event, Key, Modifiers, Shape, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_ui::{SavedQuerySqlEligibility, preview::Preview};

const SQL: &str = concat!(
    "SELECT [Customers].[CustomerID], [Customers].[CompanyName], [Customers].[ContactName], ",
    "[Customers].[ContactTitle], [Customers].[Address], [Customers].[City], [Customers].[Region], ",
    "[Customers].[PostalCode], [Customers].[Country], [Customers].[Phone], [Customers].[Fax]\n",
    "FROM [Customers]\n",
    "WHERE ([Customers].[Country] = 'Japan' OR [Customers].[Country] = 'Germany' ",
    "OR [Customers].[Country] = 'United Kingdom') AND [Customers].[CompanyName] LIKE ",
    "'A long readable company-name criterion demonstrates wrapping without horizontal scrolling ",
    "while its complete original spelling remains selectable and copyable, including this ",
    "retained final suffix 日本語';"
);
const MAX_EDITOR_HEIGHT: f32 = 150.0;

pub(super) fn prepare(harness: &mut Harness<'_, Preview>) {
    // NO_CLONE: Borrowed selected target must be owned across mutable State inspection and
    // editor setup, preserving exact source identity.
    let target = harness
        .state()
        .state()
        .selected_saved_target()
        .unwrap()
        .clone();
    {
        let state = harness.state_mut().state_mut();
        state.saved_query_sql_edit_available = true;
        // NO_CLONE: Inspection owner retains target while the original continues into details
        // and SQL source inspection.
        state.begin_saved_query_inspection(target.clone(), INSPECTION_ID);
        assert!(state.set_saved_query_details(INSPECTION_ID, &target, vec![]));
        let request = state
            .begin_saved_query_sql_inspection(target, SQL_INSPECTION_ID)
            .unwrap();
        assert!(state.set_saved_query_sql_inspection(
            SQL_INSPECTION_ID,
            &request,
            Some(SQL.into()),
            SavedQuerySqlEligibility::Unavailable,
        ));
        assert!(!state.can_edit_saved_query_sql());
        assert!(!state.can_save_active_object());
    }
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    harness.run();
    let label = harness.state().state().localizer.text("query-title");
    let editor = harness.get_by_role_and_label(Role::MultilineTextInput, &label);
    assert!(!editor.accesskit_node().is_disabled());
    assert!(editor.accesskit_node().data().is_read_only());
    for action in [
        egui::accesskit::Action::SetValue,
        egui::accesskit::Action::ReplaceSelectedText,
    ] {
        assert!(!editor.accesskit_node().data().supports_action(action));
    }
    assert_eq!(editor.value().as_deref(), Some(SQL));
    editor.focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(Event::Copy);
    harness.run_steps(1);
    assert!(
        harness
            .output()
            .platform_output
            .commands
            .iter()
            .any(|command| {
                matches!(command, egui::OutputCommand::CopyText(text) if text == SQL)
            })
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    harness.key_press(Key::ArrowLeft);
    harness.run_steps(LAYOUT_SETTLE_STEPS);
    harness.remove_cursor();
    let state = harness.state().state();
    let (original, proposed, _, eligibility) = state.saved_query_sql().unwrap();
    assert_eq!(original, SQL);
    assert_eq!(proposed, SQL);
    assert_eq!(eligibility, SavedQuerySqlEligibility::Unavailable);
    assert!(!state.saved_query_sql_dirty());
    assert!(!state.saved_query_sql_pending());
    assert!(state.saved_query_sql_error().is_none());
    assert!(!state.saved_query_sql_save_open());
    assert!(!state.can_save_active_object());
    let (galley, clip) = harness
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            Shape::Text(text) if text.galley.job.text == SQL => {
                Some((&text.galley, shape.clip_rect))
            }
            _ => None,
        })
        .expect("complete ordinary source SQL is retained in the painted galley");
    assert!(galley.job.wrap.max_width.is_finite());
    assert!(galley.rows.len() > SQL.lines().count());
    assert!(clip.height() <= MAX_EDITOR_HEIGHT + 2.0);
    assert_painted(harness, &label);
}
