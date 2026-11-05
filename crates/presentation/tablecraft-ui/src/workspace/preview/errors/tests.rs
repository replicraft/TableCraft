use super::*;
use crate::Action;

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
