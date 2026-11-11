//! Identically named result sets retain object-local column geometry.
use super::*;
use crate::workspace::objects::{ObjectKind, ObjectTab, ObjectTarget};
use crate::{Data, Database, SavedQuery, SavedQueryTarget};
use egui::{Event, PointerButton, Rect, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn saved_result(state: &mut State, object_id: u128, request: u64) -> ObjectTab {
    let name = format!("Saved {object_id}");
    let tab = state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::SavedQuery {
                object_id,
                name: name.clone(),
            },
        })
        .unwrap()
        .tab;
    let target = SavedQueryTarget {
        document_revision: 7,
        object_id,
        name,
    };
    state.begin_saved_query_inspection(target.clone(), request);
    assert!(state.set_saved_query_details(request, &target, Vec::new()));
    state.begin_saved_query_run(&target, Vec::new(), request + 1);
    assert!(state.set_saved_query_data(
        request + 1,
        &target,
        Data {
            query_contents: None,
            row_ordinals: None,
            source_revision: None,
            table: "Query".into(),
            offset: 0,
            columns: vec!["Value".into(), "Other".into()],
            rows: vec![vec!["same result text".into(), "tail".into()]],
            has_more: false,
        }
    ));
    tab
}

fn first_cell(harness: &Harness<'_, State>) -> Rect {
    harness
        .get_all_by_role(Role::Label)
        .find_map(|node| {
            node.accesskit_node()
                .value()
                .filter(|value| value.contains("same result text"))
                .map(|_| node.rect())
        })
        .expect("actual visible saved-result cell")
}

#[test]
fn saved_results_with_same_table_name_keep_independent_column_widths_across_activation() {
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(Database {
        title_path: None,
        document_revision: 7,
        title: "Independent result layout".into(),
        format: "ACE16".into(),
        tables: Vec::new(),
        queries: [1, 2]
            .into_iter()
            .map(|object_id| SavedQuery {
                object_id,
                name: format!("Saved {object_id}"),
                system: false,
            })
            .collect(),
    });
    let first = saved_result(&mut state, 1, 10);
    let second = saved_result(&mut state, 2, 20);
    assert!(state.activate_object(&first));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 740.0))
        .build_ui_state(
            |ui, state: &mut State| {
                assert!(state.show(ui).is_empty());
            },
            state,
        );
    crate::configure(&harness.ctx);
    harness.run();
    let original = first_cell(&harness);
    let start = egui::pos2(original.right(), original.center().y);
    harness.event(Event::PointerMoved(start));
    harness.run();
    harness.event(Event::PointerButton {
        pos: start,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    let end = start + egui::vec2(90.0, 0.0);
    harness.event(Event::PointerMoved(end));
    harness.run();
    harness.event(Event::PointerButton {
        pos: end,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    let resized = first_cell(&harness);
    assert!(resized.width() > original.width() + 60.0);

    assert!(harness.state_mut().activate_object(&second));
    harness.run();
    let independent = first_cell(&harness);
    assert!((independent.width() - original.width()).abs() < 1.0);

    assert!(harness.state_mut().activate_object(&first));
    harness.run();
    let revisited = first_cell(&harness);
    assert!((revisited.width() - resized.width()).abs() < 1.0);
}
