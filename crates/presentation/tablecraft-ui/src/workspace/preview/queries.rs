//! Query lifecycle samples use only existing presentation publication contracts.
use super::{Preview, fixtures};
use crate::workspace::objects::{ObjectKind, ObjectTarget};
use crate::{ParameterInput, ParameterSpec, SavedQueryTarget};

const SCRATCH_SQL: &str = "PARAMETERS [City filter] TEXT(64), [Minimum balance] CURRENCY; \
    SELECT * FROM [Customers] WHERE [City] = [City filter] AND [Balance] >= [Minimum balance]";

impl Preview {
    pub(super) fn prepare_scratch_parameters(&mut self, inspecting: bool) {
        self.mock_open_object(ObjectTarget {
            document_revision: fixtures::REVISION,
            kind: ObjectKind::Sql,
        });
        assert!(self.state.replace_query_sql(SCRATCH_SQL.into()));
        assert!(
            self.state
                .set_query_data(Some(fixtures::SQL.into()), fixtures::query())
        );

        let sql = self
            .state
            .capture_query_draft()
            .expect("bounded synthetic SQL");
        let request = self.request();
        assert!(self.state.begin_query_parameter_inspection(request, sql));
        if inspecting {
            self.state.busy = true;
            return;
        }

        let declarations = vec![
            ParameterSpec {
                name: "City filter".into(),
                kind: "text".into(),
                label: "TEXT(64)".into(),
                nullable: true,
            },
            ParameterSpec {
                name: "Minimum balance".into(),
                kind: "currency".into(),
                label: "CURRENCY".into(),
                nullable: true,
            },
        ];
        assert!(
            self.state
                .set_query_parameter_details(request, SCRATCH_SQL, declarations)
                .is_none()
        );
        assert!(self.state.scratch_parameter_prompt_open());
    }

    pub(super) fn prepare_query_running(&mut self) {
        self.mock_open_object(ObjectTarget {
            document_revision: fixtures::REVISION,
            kind: ObjectKind::Sql,
        });
        self.state
            .set_query_data(Some(fixtures::SQL.into()), fixtures::query());
        self.state.busy = true;
    }

    pub(super) fn prepare_query_error(&mut self) {
        self.mock_open_object(ObjectTarget {
            document_revision: fixtures::REVISION,
            kind: ObjectKind::Sql,
        });
        self.state
            .set_query_data(Some(fixtures::SQL.into()), fixtures::query());
        self.state.error = Some(self.state.localizer.text("query-mock-error"));
    }

    pub(super) fn prepare_saved_inspecting(&mut self) {
        let request = self.request();
        self.state
            .begin_saved_query_inspection(fixtures::saved_target(), request);
        self.state.busy = true;
    }

    pub(super) fn prepare_saved_running(&mut self) {
        let (target, inputs) = self.successful_saved_result();
        let request = self.request();
        self.state.begin_saved_query_run(target, inputs, request);
        self.state.busy = true;
    }

    pub(super) fn prepare_saved_error(&mut self) {
        self.prepare_saved_error_message(self.state.localizer.text("query-mock-error"));
    }

    pub(super) fn prepare_saved_error_message(&mut self, message: String) {
        let (target, inputs) = self.successful_saved_result();
        let request = self.request();
        self.state
            .begin_saved_query_run(target.clone(), inputs, request);
        self.state.fail_saved_query(request, target, message);
    }

    pub(super) fn prepare_saved_parameters(&mut self) {
        let target = fixtures::saved_target();
        let request = self.request();
        self.state
            .begin_saved_query_inspection(target.clone(), request);
        assert!(
            self.state
                .set_saved_query_details(request, target, typed_parameters()),
            "synthetic parameter declarations must match the selected saved query"
        );
        assert!(
            self.state.request_saved_query_run().is_none(),
            "synthetic parameter acquisition must stage without dispatching a query"
        );
        assert!(self.state.saved_query_prompt_open());
    }

    fn successful_saved_result(&mut self) -> (SavedQueryTarget, Vec<ParameterInput>) {
        let target = fixtures::saved_target();
        self.inspect_saved(target.clone());
        let inputs = fixtures::parameters(&target)
            .into_iter()
            .map(|spec| ParameterInput {
                name: spec.name,
                kind: spec.kind,
                text: String::new(),
                is_null: false,
            })
            .collect::<Vec<_>>();
        assert!(
            self.state.can_run_saved_query(&target, &inputs),
            "synthetic saved query must admit its initial text parameter"
        );
        let request = self.request();
        self.state
            .begin_saved_query_run(target.clone(), inputs.clone(), request);
        let mut data = fixtures::query();
        data.table = target.name.clone();
        assert!(
            self.state
                .set_saved_query_data(request, target.clone(), data),
            "synthetic saved results must satisfy the complete publication contract"
        );
        (target, inputs)
    }
}

fn typed_parameters() -> Vec<ParameterSpec> {
    vec![
        ParameterSpec {
            name: "Value choice".into(),
            kind: "value".into(),
            label: "VALUE".into(),
            nullable: true,
        },
        ParameterSpec {
            name: "Minimum orders".into(),
            kind: "int32".into(),
            label: "LONG".into(),
            nullable: false,
        },
        ParameterSpec {
            name: "From date".into(),
            kind: "date".into(),
            label: "DATETIME".into(),
            nullable: true,
        },
    ]
}

#[cfg(test)]
mod tests;
