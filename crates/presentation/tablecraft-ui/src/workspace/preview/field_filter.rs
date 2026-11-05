//! Invented field metadata exercises common-filter controls without native scans.
use super::{Preview, fixtures};
use crate::{
    FieldFilterDescription, FieldFilterOperator as Op, FieldFilterPatternMode, FieldFilterTarget,
    InputHint,
};

impl Preview {
    pub(super) fn mock_describe_field_filter(&mut self, target: FieldFilterTarget) {
        let request = self.request();
        if !self.state.begin_field_filter(target.clone(), request) {
            return;
        }
        let schema = fixtures::schema(&target.table);
        let native_type = schema
            .fields
            .iter()
            .find(|field| field.name == target.column)
            .map(|field| field.native_type);
        let input_hint = match native_type {
            Some(4) => InputHint::Decimal,
            Some(5) if target.column != "AnnualTotal" => InputHint::Decimal,
            Some(8) => InputHint::DateTimeIso,
            Some(10) => InputHint::Text,
            _ => {
                self.state.fail_field_filter(
                    request,
                    self.state.localizer.text("filtering-field-mock-error"),
                );
                return;
            }
        };
        let text = input_hint == InputHint::Text;
        let operators = if text {
            vec![
                Op::Equal,
                Op::NotEqual,
                Op::BeginsWith,
                Op::EndsWith,
                Op::Contains,
                Op::IsNull,
                Op::IsNotNull,
            ]
        } else {
            vec![
                Op::Equal,
                Op::NotEqual,
                Op::Less,
                Op::LessOrEqual,
                Op::Greater,
                Op::GreaterOrEqual,
                Op::Between,
                Op::NotBetween,
                Op::IsNull,
                Op::IsNotNull,
            ]
        };
        self.state.set_field_filter_description(
            request,
            FieldFilterDescription {
                target,
                input_hint,
                nullable: true,
                operators,
                pattern_mode: text.then_some(FieldFilterPatternMode::Ansi89),
            },
        );
    }

    pub(super) fn mock_apply_field_filter(
        &mut self,
        target: FieldFilterTarget,
        operator: Op,
        inputs: Vec<String>,
    ) {
        let request = self.request();
        if self
            .state
            .begin_field_filter_apply(request, target, operator, inputs)
        {
            self.state.fail_field_filter(
                request,
                self.state.localizer.text("filtering-field-mock-error"),
            );
        }
    }

    pub(super) fn prepare_field_filter_scene(&mut self, error: bool) {
        self.mock_describe_field_filter(FieldFilterTarget {
            document_revision: fixtures::REVISION,
            table: "Customers".into(),
            column: "City".into(),
            filter_id: None,
            enabled: false,
        });
        if error {
            let target = FieldFilterTarget {
                document_revision: fixtures::REVISION,
                table: "Customers".into(),
                column: "City".into(),
                filter_id: None,
                enabled: false,
            };
            self.mock_apply_field_filter(target, Op::Equal, vec![String::new()]);
        }
    }
}
