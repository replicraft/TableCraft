use super::*;
use crate::workspace::objects::ObjectTarget;
use crate::{Data, Database, FilterRestore, Schema, Table};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn missing_publications_show_loading_then_retry_without_confusing_loaded_empty_objects() {
    for design in [false, true] {
        let mut state = State::default();
        state.dismiss_backstage();
        state.database = Some(Database {
            title_path: None,
            document_revision: 7,
            title: "Object publication states".into(),
            format: "ACE16".into(),
            tables: vec![Table {
                local: true,
                name: "Items".into(),
                system: false,
                storage: "Local".into(),
            }],
            queries: Vec::new(),
        });
        let target = ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Table {
                name: "Items".into(),
            },
        };
        let tab = state.prepare_object_open(target.clone()).unwrap().tab;
        if design {
            state.prepare_object_design(&tab).unwrap();
        }
        state.busy = true;
        let mut harness = Harness::builder()
            .with_size(egui::vec2(900.0, 580.0))
            .build_ui_state(
                |ui, state: &mut (State, Vec<Action>)| {
                    state.1.extend(state.0.show(ui));
                },
                (state, Vec::new()),
            );
        crate::configure(&harness.ctx);
        harness.run_steps(3);
        assert!(harness.query_by_label("Loading this object…").is_some());
        assert!(
            harness
                .query_by_label("Your data. A familiar workspace.")
                .is_none()
        );
        assert!(
            harness
                .query_by_role_and_label(Role::Button, "Retry loading")
                .is_none()
        );
        assert!(harness.state().1.is_empty());

        harness.state_mut().0.busy = false;
        harness.run();
        assert!(
            harness
                .query_by_label("This object has not been loaded.")
                .is_some()
        );
        if !design {
            let state = &mut harness.state_mut().0;
            assert!(state.begin_filter_restore(
                80,
                FilterRestore {
                    document_revision: 7,
                    table: "Items".into(),
                    enabled: true,
                    offset: 0,
                    sort: None,
                    filter_count: 1,
                }
            ));
            state.fail_filter(80, "Literal lazy filter restore diagnosis".into());
            assert!(state.error.is_none(), "filter owns its existing diagnosis");
            assert_eq!(
                state.filter_error_message(),
                Some("Literal lazy filter restore diagnosis")
            );
            harness.run();
            assert!(
                harness
                    .query_by_label("This object could not be loaded.")
                    .is_some()
            );
            assert!(
                harness
                    .query_by_role_and_label(Role::Button, "Retry loading")
                    .is_some()
            );
            harness.state_mut().0.reset_table_filter();
        }
        harness.state_mut().0.error = Some("Literal native read diagnosis".into());
        harness.run();
        assert!(
            harness
                .query_by_label("This object could not be loaded.")
                .is_some()
        );
        harness
            .get_by_role_and_label(Role::Button, "Retry loading")
            .click();
        harness.run();
        assert!(
            matches!(harness.state().1.as_slice(), [Action::RetryObject { tab: actual }] if actual == &tab)
        );

        assert!(harness.state_mut().0.close_object(&tab));
        let replacement = harness
            .state_mut()
            .0
            .prepare_object_open(target)
            .unwrap()
            .tab;
        assert!(!harness.state().0.can_retry_object(&tab));
        assert!(harness.state().0.can_retry_object(&replacement));
        if design {
            harness
                .state_mut()
                .0
                .prepare_object_design(&replacement)
                .unwrap();
            harness.state_mut().0.set_schema(Schema {
                table: "Items".into(),
                declared_rows: 0,
                fields: Vec::new(),
            });
        } else {
            assert!(harness.state_mut().0.set_table_data(Data {
                query_contents: None,
                row_ordinals: None,
                source_revision: Some(7),
                table: "Items".into(),
                offset: 0,
                columns: vec!["Field".into()],
                rows: Vec::new(),
                has_more: false,
            }));
        }
        harness.state_mut().1.clear();
        harness.run();
        assert!(
            harness
                .query_by_role_and_label(Role::Button, "Retry loading")
                .is_none()
        );
        assert!(
            harness
                .query_by_label(if design {
                    "No fields in the captured declaration."
                } else {
                    "Field"
                })
                .is_some()
        );
        assert!(harness.query_by_label("Loading this object…").is_none());
        assert!(!harness.state().0.can_retry_object(&replacement));
        assert!(harness.state().1.is_empty());
    }
}
