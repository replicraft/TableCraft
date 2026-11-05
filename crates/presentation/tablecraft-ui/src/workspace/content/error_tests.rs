use crate::preview::{Preview, Scene};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn long_report_and_sql_keep_run_and_previous_complete_results_visible() {
    let mut preview = Preview::new(Scene::Query);
    let mut state = std::mem::take(preview.state_mut());
    let sql = "SELECT 1\n".repeat(80);
    let report = "Literal read failure; preserve the previous complete result.\n".repeat(80);
    state.sql = sql.clone();
    state.error = Some(report.clone());
    let data = state.query_data.as_ref().unwrap();
    let original = (
        data.table.clone(),
        data.offset,
        data.columns.clone(),
        data.rows[0][0].text.clone(),
    );
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .with_max_steps(32)
        .build_ui_state(
            |ui, state: &mut super::State| assert!(state.show(ui).is_empty()),
            state,
        );
    crate::configure(&harness.ctx);
    harness.run();
    let run = harness.get_by_role_and_label(Role::Button, "Run query");
    assert!(run.rect().top() >= 0.0 && run.rect().bottom() <= 550.0);
    let visible_result = harness.output().shapes.iter().any(|shape| {
        let egui::Shape::Text(text) = &shape.shape else {
            return false;
        };
        text.galley.job.text == original.3
            && shape
                .clip_rect
                .contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size()))
    });
    assert!(visible_result, "previous complete result remains visible");
    let state = harness.state();
    let data = state.query_data.as_ref().unwrap();
    assert_eq!(
        (
            data.table.clone(),
            data.offset,
            data.columns.clone(),
            data.rows[0][0].text.clone(),
        ),
        original,
    );
    assert_eq!(state.sql, sql);
    assert_eq!(state.error.as_deref(), Some(report.as_str()));
}
