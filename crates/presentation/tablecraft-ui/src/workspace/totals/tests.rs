use super::*;
use crate::preview::{Preview, Scene};
use crate::{HorizontalAlignment, Locale, Localizer};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_display::{CurrencyChoice, NativeFormat, ScalarPresentation};

fn metadata(state: &mut State) -> TotalsTarget {
    state.totals.session = Some(Box::new(Session::default()));
    let Some(Action::DescribeTotals { target }) = state.take_totals_request() else {
        panic!("enabling Totals requests metadata only");
    };
    assert!(
        state.take_totals_request().is_none(),
        "queued intent is emitted once"
    );
    assert!(state.begin_totals_description(&target, 1));
    let fields = state
        .data
        .as_ref()
        .unwrap()
        .columns
        .iter()
        .map(|column| TotalsField {
            column: column.clone(),
            aggregates: if column.name == "Balance" {
                vec![TotalAggregate::Count, TotalAggregate::Average]
            } else {
                vec![TotalAggregate::Count]
            },
        })
        .collect();
    assert!(state.set_totals_description(
        1,
        TotalsDescription {
            target: target.clone(),
            fields
        }
    ));
    assert!(
        state.take_totals_request().is_none(),
        "default None choices do not scan"
    );
    target
}

fn select(
    state: &mut State,
    target: &TotalsTarget,
    column: &str,
    aggregate: TotalAggregate,
) -> Vec<TotalSpec> {
    state.apply_total_choices(vec![Choice {
        target: target.clone(),
        column: column.into(),
        aggregate: Some(aggregate),
    }]);
    let Some(Action::ComputeTotals { specs, .. }) = state.take_totals_request() else {
        panic!("explicit aggregate choice requests computation");
    };
    specs
}

#[test]
fn totals_failures_stop_requeue_and_hide_keeps_exact_cancellation_authority() {
    let mut preview = Preview::new(Scene::Datasheet);
    let state = preview.state_mut();
    let target = metadata(state);
    let specs = select(state, &target, "CustomerID", TotalAggregate::Count);
    assert!(state.begin_totals_compute(2, &target, specs.clone()));
    assert!(state.set_totals_result(
        2,
        TotalsData {
            target: target.clone(),
            specs: specs.clone(),
            cells: vec![Cell::scalar("205")],
            source_rows: 205,
            view_rows: 205,
        }
    ));
    assert!(state.begin_totals_compute(5, &target, specs.clone()));
    state.fail_totals(5, "Literal aggregate failure".into());
    assert!(state.take_totals_request().is_none());
    assert_eq!(state.totals_cell("CustomerID").unwrap().text, "205");
    assert!(state.selected_total_cell().is_none());
    assert_eq!(
        state.totals.session.as_ref().unwrap().error.as_deref(),
        Some("Literal aggregate failure")
    );

    assert!(state.toggle_totals_visibility().is_none());
    assert!(state.totals_cell("CustomerID").is_none());
    assert!(state.toggle_totals_visibility().is_none());
    let Some(Action::ComputeTotals { specs, .. }) = state.take_totals_request() else {
        panic!("showing a failed result must request fresh totals");
    };
    assert!(state.begin_totals_compute(3, &target, specs.clone()));
    let rows = state.data.as_ref().unwrap().rows.as_ptr();
    let hide = state.toggle_totals_visibility();
    assert!(matches!(hide, Some(Action::CancelTotals { request_id: 3 })));
    assert_eq!(state.totals_request_id(), Some(3));
    assert!(!state.set_totals_result(
        3,
        TotalsData {
            target: target.clone(),
            specs: specs.clone(),
            cells: vec![Cell::scalar("205")],
            source_rows: 205,
            view_rows: 205,
        }
    ));
    assert_eq!(
        state.totals_request_id(),
        Some(3),
        "late reply cannot consume cancellation authority"
    );
    assert!(!state.cancel_totals(9));
    assert!(state.cancel_totals(3));
    assert!(state.take_totals_request().is_none());
    assert_eq!(state.data.as_ref().unwrap().rows.as_ptr(), rows);

    state.toggle_totals_visibility();
    let Some(Action::ComputeTotals { specs, .. }) = state.take_totals_request() else {
        panic!("showing the retained choice can retry explicitly");
    };
    assert!(state.begin_totals_compute(4, &target, specs));
    let data = state.data.take().unwrap();
    assert!(state.set_table_data(data));
    assert!(!state.set_totals_result(
        4,
        TotalsData {
            target,
            specs: vec![TotalSpec {
                column: "CustomerID".into(),
                aggregate: TotalAggregate::Count
            }],
            cells: vec![Cell::scalar("999")],
            source_rows: 205,
            view_rows: 205,
        }
    ));
    assert!(
        state.totals_cell("CustomerID").is_none(),
        "old publication never supplies totals"
    );
}

#[test]
fn page_changes_and_parked_display_preferences_preserve_typed_full_view_totals() {
    let mut preview = Preview::new(Scene::Datasheet);
    let state = preview.state_mut();
    state.formatting.system_locale = Some("en-US".into());
    let target = metadata(state);
    let specs = select(state, &target, "Balance", TotalAggregate::Average);
    assert!(state.begin_totals_compute(2, &target, specs.clone()));
    let facts = ScalarPresentation::CurrencyAverage {
        bits: 0.1_f64.to_bits(),
        native_format: NativeFormat::Currency,
    };
    let mut mean = Cell::scalar("0.1");
    mean.presentation = Some(facts);
    mean.alignment = HorizontalAlignment::Right;
    assert!(state.set_totals_result(
        2,
        TotalsData {
            target: target.clone(),
            specs,
            cells: vec![mean],
            source_rows: 205,
            view_rows: 205
        }
    ));
    state.apply_totals_selection(Some(crate::datasheet::SelectionOwner::Total {
        target,
        column: "Balance".into(),
    }));
    let rows = state
        .totals
        .session
        .as_ref()
        .unwrap()
        .result
        .as_ref()
        .unwrap()
        .cells
        .as_ptr();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 740.0))
        .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
    crate::configure(&harness.ctx);
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Page tools")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Next page ›")
        .click();
    harness.run();
    let state = harness.state().state();
    assert_eq!(state.data.as_ref().unwrap().offset, 100);
    assert_eq!(
        state
            .totals
            .session
            .as_ref()
            .unwrap()
            .result
            .as_ref()
            .unwrap()
            .cells
            .as_ptr(),
        rows
    );
    assert!(
        state.totals.session.as_ref().unwrap().error.is_none(),
        "ordinary paging never rescans or invokes mock computation"
    );

    harness
        .get_by_role_and_label(Role::Button, "Orders")
        .click();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(
        harness.state().state().data.as_ref().unwrap().table,
        "Orders"
    );
    let state = harness.state_mut().state_mut();
    state.formatting.currency = CurrencyChoice::Raw;
    state.localizer = Localizer::new(Locale::De).unwrap();
    harness.run();
    harness
        .get_by_role_and_label(Role::Tab, "Customers")
        .click();
    harness.run();
    let state = harness.state_mut().state_mut();
    assert_eq!(state.formatting.system_locale.as_deref(), Some("en-US"));
    assert_eq!(state.totals_cell("Balance").unwrap().text, "0.1");
    assert_eq!(
        state.totals_cell("Balance").unwrap().presentation,
        Some(facts)
    );
    assert_eq!(
        state
            .totals
            .session
            .as_ref()
            .unwrap()
            .result
            .as_ref()
            .unwrap()
            .cells
            .as_ptr(),
        rows
    );
    assert!(state.take_totals_request().is_none());
}

#[test]
fn totals_refuses_a_thirty_third_choice_and_exposes_toggle_state() {
    let mut preview = Preview::new(Scene::Datasheet);
    let state = preview.state_mut();
    let mut data = state.data.take().unwrap();
    data.columns = (0..33)
        .map(|index| format!("Field{index}").into())
        .collect();
    data.rows.clear();
    data.offset = 0;
    data.has_more = false;
    assert!(state.set_table_data(data));
    let target = metadata(state);
    let choices = (0..32)
        .map(|index| Choice {
            target: target.clone(),
            column: format!("Field{index}"),
            aggregate: Some(TotalAggregate::Count),
        })
        .collect();
    state.apply_total_choices(choices);
    let retained = state.totals.session.as_ref().unwrap().specs.clone();
    assert_eq!(retained.len(), 32);

    state.apply_total_choices(vec![Choice {
        target,
        column: "Field32".into(),
        aggregate: Some(TotalAggregate::Count),
    }]);
    let session = state.totals.session.as_ref().unwrap();
    assert_eq!(session.specs, retained);
    assert_eq!(
        session.error.as_deref(),
        Some(state.localizer.text("totals-limit").as_str())
    );
    assert!(state.take_totals_request().is_none());
    assert!(state.totals_request_id().is_none());

    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 740.0))
        .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
    crate::configure(&harness.ctx);
    harness.run();
    let toggle = harness.get_by_role_and_label(Role::Button, "Totals");
    assert_eq!(
        toggle.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    toggle.click();
    harness.run();
    assert_eq!(
        harness
            .get_by_role_and_label(Role::Button, "Totals")
            .accesskit_node()
            .toggled(),
        Some(egui::accesskit::Toggled::False)
    );
}
