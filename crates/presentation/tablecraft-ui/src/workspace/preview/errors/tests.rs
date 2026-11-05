use super::*;
use crate::{Action, RecordTarget};

#[test]
fn long_service_reports_preserve_complete_sources_and_retry_inputs() {
    for scene in [
        Scene::LongReadError,
        Scene::LongQueryError,
        Scene::LongSavedError,
    ] {
        let preview = Preview::new(scene);
        let state = preview.state();
        let parent = state.data.as_ref().unwrap();
        assert_eq!(parent.table, "Customers");
        assert_eq!(
            parent.source_revision,
            Some(super::super::fixtures::REVISION)
        );
        assert!(!parent.rows.is_empty());
        assert_eq!(state.sql, super::super::fixtures::SQL);
        assert!(!state.busy);
        if scene == Scene::LongSavedError {
            let Some(Action::RunSavedQuery { target, parameters }) = state.saved_query_run_action()
            else {
                panic!("failed saved run retains its valid parameter choices");
            };
            assert_eq!(target, super::super::fixtures::saved_target());
            assert_eq!(parameters.len(), 1);
            assert_eq!(parameters[0].name, "Enter a city");
            assert_eq!(parameters[0].kind, "text");
            assert_eq!(parameters[0].text, "");
            assert!(!parameters[0].is_null);
            let result = state.saved_query.data.as_ref().unwrap();
            assert_eq!(result.table, target.name);
            assert_eq!(result.source_revision, None);
            assert!(!result.rows.is_empty());
            assert!(state.query_data.is_none());
        } else {
            assert_eq!(state.error.as_deref(), Some(SERVICE_REPORT));
            if scene == Scene::LongQueryError {
                let result = state.query_data.as_ref().unwrap();
                assert_eq!(result.source_revision, None);
                assert!(!result.rows.is_empty());
            } else {
                assert!(state.query_data.is_none());
            }
        }
        assert_eq!(state.localizer.diagnostics(), Default::default());
    }
}

#[test]
fn successful_mock_read_and_query_retries_clear_the_previous_global_report() {
    for scene in [
        Scene::Error,
        Scene::LongReadError,
        Scene::QueryError,
        Scene::LongQueryError,
    ] {
        for action in [
            Action::Query,
            Action::Page {
                table: "Customers".into(),
                offset: 0,
            },
            Action::ViewSchema {
                table: "Customers".into(),
            },
        ] {
            let mut preview = Preview::new(scene);
            assert!(preview.state.error.is_some());
            let sql = preview.state.sql.clone();
            preview.dispatch(action);
            assert!(
                preview.state.error.is_none(),
                "successful mock response replaces the prior failure"
            );
            assert_eq!(preview.state.sql, sql);
            let table = preview.state.data.as_ref().unwrap();
            assert_eq!(table.table, "Customers");
            assert!(!table.rows.is_empty());
            assert!(preview.state.mock_preview);
        }
    }
}

#[test]
fn admitted_mock_retries_clear_cancel_reports_but_stale_targets_preserve_them() {
    for retry in ["saved run", "saved inspection", "record"] {
        let mut preview = Preview::new(if retry == "record" {
            Scene::Datasheet
        } else {
            Scene::SavedRunning
        });
        preview.dispatch(Action::Cancel);
        let report = preview.state.error.clone();
        assert!(report.is_some());
        let sql = preview.state.sql.clone();
        if retry == "record" {
            let data = preview.state.data.as_ref().unwrap();
            let target = RecordTarget {
                document_revision: data.source_revision.unwrap(),
                table: data.table.clone(),
                parent_ordinal: data.offset + 7,
                column: data.columns[0].name.clone(),
            };
            let mut stale = target.clone();
            stale.document_revision += 1;
            preview.dispatch(Action::NavigateRecord { target: stale });
            assert_eq!(preview.state.error, report);
            let ordinal = target.parent_ordinal;
            preview.dispatch(Action::NavigateRecord { target });
            assert_eq!(
                preview.state.data.as_ref().unwrap().offset,
                ordinal,
                "mock publication starts at the requested non-page-boundary record"
            );
            assert_eq!(preview.state.data.as_ref().unwrap().rows[0][0].text, "1008");
        } else {
            let Some(Action::RunSavedQuery { target, parameters }) =
                preview.state.saved_query_run_action()
            else {
                panic!("cancelled saved run retains its ready inputs");
            };
            let mut stale = target.clone();
            stale.document_revision += 1;
            if retry == "saved run" {
                preview.dispatch(Action::RunSavedQuery {
                    target: stale,
                    parameters: parameters.clone(),
                });
                assert_eq!(preview.state.error, report);
                preview.dispatch(Action::RunSavedQuery { target, parameters });
            } else {
                preview.dispatch(Action::InspectSavedQuery { target: stale });
                assert_eq!(preview.state.error, report);
                preview.dispatch(Action::InspectSavedQuery { target });
            }
        }
        assert!(
            preview.state.error.is_none(),
            "admitted {retry} replaces the cancellation report"
        );
        assert_eq!(preview.state.sql, sql);
        assert!(!preview.state.data.as_ref().unwrap().rows.is_empty());
        assert_eq!(preview.state.localizer.diagnostics(), Default::default());
    }
}
