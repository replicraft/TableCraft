//! Parser limits reject real Field input without replacing current authoring data.
use super::*;
use crate::{Cell, Data};
use egui_kittest::kittest::NodeT;

#[test]
fn real_field_paste_limits_preserve_complete_and_unresolved_edits_without_actions() {
    let deep = format!("Rejected: {}1{}", "(".repeat(33), ")".repeat(33));
    let tokens = format!("Rejected: {}1", "1+".repeat(600));
    for unresolved in [false, true] {
        let mut initial = state();
        assert!(initial.set_query_data(
            None,
            Data {
                query_contents: None,
                source_revision: None,
                row_ordinals: None,
                table: "Previous results".into(),
                offset: 0,
                columns: vec!["Value".into()],
                rows: vec![vec![Cell::scalar("retained")]],
                has_more: false,
            }
        ));
        let mut harness = harness(initial);
        edit(&mut harness, 0, "Chosen: CustomerID");
        if unresolved {
            edit(&mut harness, 0, "Chosen: IIf(");
        }
        for candidate in [&deep, &tokens] {
            assert!(candidate.len() < MAX_DESIGN_TEXT_BYTES);
            let request = harness.state().query_design_request().unwrap();
            // NO_CLONE: The expected pre-paste model must remain independently
            // owned while the real editor attempts to replace its current input.
            let draft = harness.state().query_design_draft().unwrap().clone();
            // NO_CLONE: The exact previous raw input is the independent expected
            // value for the following attempted replacement and render pass.
            let previous_raw = raw(harness.state(), 0).unwrap().to_owned();
            let rows = harness.state().query_data.as_ref().unwrap().rows.as_ptr();
            harness.ctx.data_mut(|data| {
                data.insert_temp(egui::Id::new("qbe_test_emitted_actions"), false);
            });

            edit(&mut harness, 0, candidate);

            assert_eq!(
                harness.state().query_design_request().as_ref(),
                Some(&request)
            );
            assert_eq!(harness.state().query_design_draft(), Some(&draft));
            assert_eq!(raw(harness.state(), 0), Some(previous_raw.as_str()));
            assert_eq!(
                harness
                    .get_all_by_role_and_label(Role::TextInput, "Field expression")
                    .next()
                    .unwrap()
                    .accesskit_node()
                    .value()
                    .as_deref(),
                Some(previous_raw.as_str())
            );
            assert_eq!(
                harness.state().has_unresolved_query_design_edits(),
                unresolved
            );
            assert_eq!(
                harness.state().query_data.as_ref().unwrap().rows.as_ptr(),
                rows
            );
            assert_eq!(
                harness.state().query_data.as_ref().unwrap().rows[0][0].text,
                "retained"
            );
            assert!(harness.state().query_design_request_id().is_none());
            assert!(!harness.state().busy);
            assert_eq!(
                harness
                    .state()
                    .query_design
                    .session
                    .as_ref()
                    .unwrap()
                    .error
                    .as_deref(),
                Some(harness.state().localizer.text("qbe-limit").as_str())
            );
            assert!(!harness.ctx.data(|data| {
                data.get_temp::<bool>(egui::Id::new("qbe_test_emitted_actions"))
                    .expect("the existing harness records emitted actions")
            }));
        }
    }
}
