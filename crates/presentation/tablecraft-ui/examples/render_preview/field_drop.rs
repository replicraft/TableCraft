//! Source-free saved-field deletion frames through the shared widgets.
//! Eligibility and request IDs here are illustrative; no native capability,
//! SaveWorker, file, journal or successful deletion is supplied by this renderer.
use super::LAYOUT_SETTLE_STEPS;
use egui::{Id, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_ui::{
    Column, Data, FieldDropConfirmationDecision, FieldDropTarget, ObjectKind, preview::Preview,
};

const INSPECTION_ID: u64 = 850;
const REVIEW_REQUEST_ID: u64 = 851;
const NATIVE_NUMBER: usize = 17;
const FIELD_NAME: &str = "Score";

pub(super) fn is_mode(mode: &str) -> bool {
    matches!(
        mode,
        "none" | "confirmation" | "inspecting" | "source-refused" | "pending" | "close" | "review"
    )
}

pub(super) fn prepare(harness: &mut Harness<'_, Preview>, mode: &str) {
    let tab = harness
        .state()
        .state()
        .active_object()
        .expect("saved table preview owner");
    let ObjectKind::Table { name } = &tab.target.kind else {
        panic!("saved-field deletion preview requires a table");
    };
    let table = name.clone();
    let revision = tab.target.document_revision;
    let tab_id = tab.id;

    let state = harness.state_mut().state_mut();
    assert!(state.set_table_data(Data {
        source_revision: Some(revision),
        row_ordinals: None,
        table: table.clone(),
        offset: 0,
        columns: vec![
            Column {
                name: "ID".into(),
                caption: None,
                native_number: Some(0),
            },
            Column {
                name: FIELD_NAME.into(),
                caption: Some("Friendly".into()),
                native_number: Some(NATIVE_NUMBER),
            },
        ],
        rows: Vec::new(),
        has_more: false,
    }));
    state.field_drop_available = true;
    assert!(state.begin_field_drop(FieldDropTarget {
        tab,
        column: FIELD_NAME.into(),
        native_number: NATIVE_NUMBER,
    }));
    let target = state
        .field_drop_inspection_target()
        .expect("exact source-free target");
    state
        .prepare_field_drop_inspection(&target)
        .expect("presentation-only inspection slot")
        .bind(INSPECTION_ID);

    if mode != "inspecting" {
        assert!(state.set_field_drop_inspection_by_id(
            INSPECTION_ID,
            revision,
            &table,
            FIELD_NAME,
            NATIVE_NUMBER,
            mode != "source-refused",
        ));
    }

    if matches!(mode, "pending" | "close" | "review") {
        assert!(
            state
                .choose_field_drop_confirmation(&target, 0, FieldDropConfirmationDecision::Yes)
                .is_some()
        );
        if mode == "close" {
            assert!(state.request_current_field_drop_close());
        } else if mode == "review" {
            let request = state
                .field_drop_review_request()
                .expect("confirmed exact local deletion");
            assert!(state.begin_field_drop_review(&request, REVIEW_REQUEST_ID));
            state.busy = true;
        }
    }

    harness.run_steps(LAYOUT_SETTLE_STEPS);
    if !matches!(mode, "inspecting" | "review") {
        harness.run();
    }
    harness.remove_cursor();

    let state = harness.state().state();
    let target = state.field_drop_target().expect("retained deletion target");
    assert_eq!(target.native_number, NATIVE_NUMBER);
    assert_eq!(target.column, FIELD_NAME);
    assert_eq!(state.active_object().unwrap().id, tab_id);
    assert_eq!(
        state.field_drop_dirty(),
        matches!(mode, "pending" | "close" | "review")
    );
    assert!(state.field_drop_save_status().is_none());
    let original = state.data.as_ref().expect("original source-free table");
    assert_eq!(original.source_revision, Some(revision));
    assert_eq!(original.columns[1].name, FIELD_NAME);
    assert_eq!(original.columns[1].native_number, Some(NATIVE_NUMBER));
    assert!(original.rows.is_empty());

    match mode {
        "confirmation" | "inspecting" | "source-refused" => {
            confirmation(harness, mode == "confirmation");
        }
        "close" => close(harness),
        "pending" => {
            let label = state.localizer.text("field-drop-close-save");
            let save = harness
                .query_all_by_role_and_label(Role::Button, &label)
                .max_by(|left, right| left.rect().top().total_cmp(&right.rect().top()))
                .expect("actual pending-deletion Save footer");
            assert!(!save.accesskit_node().is_disabled());
            assert!(harness.ctx.viewport_rect().contains_rect(save.rect()));
        }
        "review" => {
            assert!(!state.can_review_field_drop());
        }
        _ => unreachable!("validated saved-field preview mode"),
    }
}

fn confirmation(harness: &Harness<'_, Preview>, eligible: bool) {
    let modal = harness
        .ctx
        .memory(|memory| memory.area_rect(Id::new("field_drop_confirmation")))
        .expect("actual confirmation area");
    assert!(harness.ctx.viewport_rect().contains_rect(modal));
    for (key, enabled) in [("field-drop-yes", eligible), ("field-drop-no", true)] {
        let label = harness.state().state().localizer.text(key);
        let button = harness.get_by_role_and_label(Role::Button, &label);
        assert!(modal.contains_rect(button.rect()));
        assert_eq!(!button.accesskit_node().is_disabled(), enabled);
    }
}

fn close(harness: &Harness<'_, Preview>) {
    let modal = harness
        .ctx
        .memory(|memory| memory.area_rect(Id::new("field_drop_close")))
        .expect("actual pending-deletion close area");
    assert!(harness.ctx.viewport_rect().contains_rect(modal));
    for key in [
        "field-drop-close-save",
        "field-drop-close-discard",
        "field-drop-close-cancel",
    ] {
        let label = harness.state().state().localizer.text(key);
        let mut buttons = harness
            .query_all_by_role_and_label(Role::Button, &label)
            .filter(|button| modal.contains_rect(button.rect()));
        let button = buttons.next().expect("actual close decision inside modal");
        assert!(buttons.next().is_none());
        assert!(!button.accesskit_node().is_disabled());
    }
}
