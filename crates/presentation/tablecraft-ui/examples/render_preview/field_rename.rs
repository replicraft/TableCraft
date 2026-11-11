//! Source-free field-name presentation through the real shared state and widgets.
//!
//! Synthetic metadata and inspection facts grant no native review or save owner.
//! The retained datasheet reference has no expanded rename capture, so these
//! frames qualify an adaptation rather than Access interaction or save parity.
use super::LAYOUT_SETTLE_STEPS;
use egui::{Id, Shape, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::table::field_rename::FieldRenameNameProfile;
use tablecraft_ui::{
    Cell, CellKind, Column, Data, DisplayControl, EditSaveOutcome, FieldRenameSaveIdentity,
    FieldRenameTarget, HorizontalAlignment, ObjectKind, preview::Preview,
};

const INSPECTION_ID: u64 = 840;
const REVIEW_REQUEST_ID: u64 = 841;
const REVIEW_ID: u64 = 842;
// Presentation identity only. No SaveWorker, native capability, file or journal exists.
const PRESENTATION_SAVE_ID: u64 = 843;
const PRESENTATION_OUTCOME: &str =
    "Illustrative status only; no native capability, actor, file or journal.";
const REOPEN_ERROR: &str =
    "Illustrative full-image read failure; no native operation was performed.";
const NATIVE_NUMBER: usize = 17;
const ORIGINAL_NAME: &str = "Score";
const CHANGED_NAME: &str = "Total";
const UNSUPPORTED_NAME: &str = "Longer";

pub(super) fn is_mode(mode: &str) -> bool {
    matches!(
        mode,
        "none"
            | "name"
            | "unsupported"
            | "grow-name"
            | "populated-name"
            | "shrink-name"
            | "source-refused"
            | "unchanged"
            | "review"
            | "close"
            | "returned"
            | "unchanged-returned"
    )
}

pub(super) fn prepare(harness: &mut Harness<'_, Preview>, mode: &str) {
    let tab = harness
        .state()
        .state()
        .active_object()
        .expect("saved datasheet presentation owner");
    let ObjectKind::Table { name } = &tab.target.kind else {
        panic!("field rename preview requires a saved table");
    };
    let revision = tab.target.document_revision;
    let tab_id = tab.id;

    let target = {
        let state = harness.state_mut().state_mut();
        // Direct source-free facts deliberately distinguish display position 1
        // from native number 17. No file is opened to manufacture eligibility.
        // NO_CLONE: The table Data and the pending field-operation tab retain independent owned
        // table names; both identities must survive the state transition.
        assert!(state.set_table_data(Data {
            query_contents: None,
            source_revision: Some(revision),
            row_ordinals: None,
            table: name.clone(),
            offset: 0,
            columns: vec![
                Column {
                    name: "ID".into(),
                    caption: None,
                    native_number: Some(0),
                },
                Column {
                    name: ORIGINAL_NAME.into(),
                    caption: None,
                    native_number: Some(NATIVE_NUMBER),
                },
            ],
            rows: if mode == "populated-name" {
                populated_rows()
            } else {
                Vec::new()
            },
            has_more: false,
        }));
        state.field_rename_available = true;
        assert!(state.begin_field_rename(FieldRenameTarget {
            tab,
            column: ORIGINAL_NAME.into(),
            native_number: NATIVE_NUMBER,
        }));

        let target = state
            .field_rename_inspection_target()
            .expect("exact current presentation target");
        state
            .prepare_field_rename_inspection(&target)
            .expect("presentation inspection admission")
            .bind(INSPECTION_ID);
        let ObjectKind::Table { name: table } = &target.tab.target.kind else {
            unreachable!("the inspected presentation target retains its table");
        };
        let source_eligible = mode != "source-refused";
        assert!(state.set_field_rename_inspection_by_id(
            INSPECTION_ID,
            revision,
            table,
            ORIGINAL_NAME,
            NATIVE_NUMBER,
            source_eligible,
            source_eligible.then_some(
                if matches!(mode, "grow-name" | "shrink-name" | "populated-name") {
                    FieldRenameNameProfile::FittedAscii {
                        original_units: ORIGINAL_NAME.len(),
                    }
                } else {
                    FieldRenameNameProfile::SameWidthAscii {
                        units: ORIGINAL_NAME.len(),
                    }
                }
            ),
        ));
        target
    };

    let proposed = match mode {
        "unsupported" => UNSUPPORTED_NAME,
        "grow-name" | "populated-name" => "LongerScore",
        "shrink-name" => "Pts",
        "unchanged" | "unchanged-returned" | "source-refused" => ORIGINAL_NAME,
        _ => CHANGED_NAME,
    };
    if mode != "source-refused" {
        assert!(
            harness
                .state_mut()
                .state_mut()
                .set_field_rename_name(&target, proposed.into())
        );
    }

    if mode == "review" {
        let state = harness.state_mut().state_mut();
        let request = state
            .field_rename_review_request()
            .expect("exact changed presentation snapshot");
        assert!(state.begin_field_rename_review(&request, REVIEW_REQUEST_ID));
        state.busy = true;
    }

    if mode == "close" {
        assert!(
            harness
                .state_mut()
                .state_mut()
                .request_current_field_rename_close()
        );
    }
    if matches!(mode, "returned" | "unchanged-returned") {
        let state = harness.state_mut().state_mut();
        let request = state
            .field_rename_review_request()
            .expect("presentation request");
        assert!(state.begin_field_rename_review(&request, REVIEW_REQUEST_ID));
        assert!(state.accept_field_rename_review(&request, REVIEW_REQUEST_ID, REVIEW_ID));
        let identity = if mode == "unchanged-returned" {
            let prepared = state
                .prepare_unchanged_field_rename(&request, REVIEW_REQUEST_ID, REVIEW_ID)
                .expect("unchanged presentation branch");
            prepared.bind()
        } else {
            let prepared = state
                .prepare_field_rename_save(&request, REVIEW_REQUEST_ID, REVIEW_ID)
                .expect("changed presentation branch");
            let identity = prepared.bind(PRESENTATION_SAVE_ID);
            assert!(state.set_field_rename_save_outcome(
                identity,
                EditSaveOutcome::Saved(PRESENTATION_OUTCOME.into()),
            ));
            assert!(state.begin_field_rename_reopen(identity));
            identity
        };
        assert!(state.fail_field_rename_reopen(identity, REOPEN_ERROR.into()));
        assert!(state.return_field_rename_to_draft(&request, identity, None));
        assert!(state.field_rename_source_stale());
        let warning = state
            .field_rename_warning()
            .expect("retained presentation warning");
        assert_eq!(warning.reopen_error, Some(REOPEN_ERROR));
        assert!(warning.cleanup.is_none());
        if mode == "unchanged-returned" {
            assert!(matches!(
                identity,
                FieldRenameSaveIdentity::UnchangedReview { .. }
            ));
            assert!(warning.outcome.is_none());
        } else {
            assert!(matches!(warning.outcome, Some(EditSaveOutcome::Saved(_))));
        }
    }

    harness.run_steps(LAYOUT_SETTLE_STEPS);
    // Review deliberately animates; never wait for its repaint loop to go idle.
    if !harness.state().state().busy {
        harness.run();
    }
    harness.remove_cursor();

    let state = harness.state().state();
    let (actual_target, actual_name, _) = state
        .field_rename_proposal()
        .expect("rename keeps its local name owner");
    assert_eq!(actual_target.native_number, NATIVE_NUMBER);
    assert_eq!(actual_name, proposed);
    assert_eq!(state.active_object().unwrap().id, tab_id);
    assert_eq!(
        state.can_review_field_rename(),
        matches!(
            mode,
            "name" | "unchanged" | "grow-name" | "shrink-name" | "populated-name"
        ),
        "only the supported idle proposal permits a review request"
    );
    assert!(
        state.field_rename_save_status().is_none(),
        "illustrative Return has moved status into warning; no native actor exists"
    );
    let data = state.data.as_ref().expect("original synthetic table");
    assert_eq!(data.source_revision, Some(revision));
    assert_eq!(data.columns[1].name, ORIGINAL_NAME);
    if mode == "populated-name" {
        assert_eq!(data.rows.len(), 3);
        assert_eq!(data.rows[0][1].text, "42");
        assert_eq!(data.rows[2][1].text, "-7");
        assert!(matches!(
            data.rows[1][1].kind,
            CellKind::Scalar { is_null: true, .. }
        ));
    } else {
        assert!(data.rows.is_empty());
    }

    if !matches!(mode, "review" | "close") {
        let label = state.localizer.text("field-rename-name");
        let input = harness.get_by_role_and_label(Role::TextInput, &label);
        assert_eq!(input.value().as_deref(), Some(proposed));
        assert_eq!(
            input.accesskit_node().is_disabled(),
            matches!(
                mode,
                "source-refused" | "close" | "returned" | "unchanged-returned"
            ),
        );
        assert!(harness.ctx.viewport_rect().contains_rect(input.rect()));
    }
    if mode == "close" {
        let modal = area(harness, "field_rename_close");
        for key in [
            "field-rename-close-save",
            "field-rename-close-discard",
            "field-rename-close-cancel",
        ] {
            let label = harness.state().state().localizer.text(key);
            let buttons = harness
                .query_all_by_role_and_label(Role::Button, &label)
                .filter(|button| modal.contains_rect(button.rect()))
                .collect::<Vec<_>>();
            assert_eq!(
                buttons.len(),
                1,
                "one {key} belongs to the exact close modal"
            );
            assert!(!buttons[0].accesskit_node().is_disabled(), "{key}");
        }
    }
    if mode == "review" {
        area(harness, "field_rename_review");
    }
    if matches!(mode, "returned" | "unchanged-returned") {
        let key = if mode == "unchanged-returned" {
            "field-rename-warning-unchanged"
        } else {
            "field-rename-warning-saved"
        };
        painted(harness, key);
        let label = harness
            .state()
            .state()
            .localizer
            .text("field-rename-close-editor");
        let button = harness.get_by_role_and_label(Role::Button, &label);
        assert!(harness.ctx.viewport_rect().contains_rect(button.rect()));
        assert!(!harness.state().state().can_edit_field_rename());
    }
}

fn area(harness: &Harness<'_, Preview>, id: &str) -> egui::Rect {
    let rect = harness
        .ctx
        .memory(|memory| memory.area_rect(Id::new(id)))
        .expect("actual rename modal area");
    assert!(
        harness.ctx.viewport_rect().contains_rect(rect),
        "{id} rect={rect:?} must fit viewport={:?}",
        harness.ctx.viewport_rect()
    );
    rect
}

fn painted(harness: &Harness<'_, Preview>, key: &str) {
    let label = harness.state().state().localizer.text(key);
    assert!(
        harness.output().shapes.iter().any(|shape| {
            let Shape::Text(text) = &shape.shape else {
                return false;
            };
            text.galley.job.text == label
                && harness
                    .ctx
                    .viewport_rect()
                    .contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size()))
        }),
        "complete {key} is painted inside the viewport"
    );
}

fn populated_rows() -> Vec<Vec<Cell>> {
    let mut null = Cell::scalar("");
    null.kind = CellKind::Scalar {
        is_null: true,
        stored_calculation: false,
        boolean: None,
        display_control: DisplayControl::Uninspected,
    };
    let mut rows = vec![
        vec![Cell::scalar("1"), Cell::scalar("42")],
        vec![Cell::scalar("2"), null],
        vec![Cell::scalar("3"), Cell::scalar("-7")],
    ];
    for row in &mut rows {
        for cell in row {
            cell.alignment = HorizontalAlignment::Right;
        }
    }
    rows
}
