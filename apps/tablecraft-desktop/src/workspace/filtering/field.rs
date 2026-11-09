//! Field metadata and explicit predicate dispatch; native parsing stays in runtime.
use crate::view_models;
use crate::workspace::request_read;
use crate::workspace::{Desktop, PendingOperation};
use tablecraft_runtime::{
    self as runtime, Command, FieldFilterOperator as NativeOperator, FilterHandle,
};
use tablecraft_ui::{
    FieldFilterDescription, FieldFilterOperator, FieldFilterPatternMode, FieldFilterTarget,
    FilterTarget,
};

impl Desktop {
    fn field_filter_base(&self, target: &FieldFilterTarget) -> Result<Option<FilterHandle>, ()> {
        let Some(filter_id) = target.filter_id else {
            return Ok(None);
        };
        self.filter_handle(&FilterTarget {
            document_revision: target.document_revision,
            table: target.table.clone(),
            filter_id,
        })
        .map(Some)
        .ok_or(())
    }

    pub(in crate::workspace) fn describe_field_filter(&mut self, target: FieldFilterTarget) {
        if self.editing.has_save() || !self.state.can_describe_field_filter(&target) {
            return;
        }
        if self.field_filter_base(&target).is_err() {
            self.state.error = Some(self.state.localizer.text("filtering-stale"));
            return;
        }
        let Some(request_id) = request_read(
            Command::DescribeFieldFilter {
                target: native_target(&target),
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        if !self.state.begin_field_filter(target.clone(), request_id) {
            self.worker.cancel();
            return;
        }
        self.generation = request_id;
        self.pending = PendingOperation::FieldFilterDescribe { request_id, target };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn apply_field_filter(
        &mut self,
        target: FieldFilterTarget,
        operator: FieldFilterOperator,
        inputs: Vec<String>,
    ) {
        if self.editing.has_save()
            || !self
                .state
                .can_apply_field_filter(&target, operator, &inputs)
        {
            return;
        }
        let Ok(base) = self.field_filter_base(&target) else {
            self.state.error = Some(self.state.localizer.text("filtering-stale"));
            return;
        };
        let Some(request_id) = request_read(
            Command::ApplyFieldFilter {
                target: native_target(&target),
                operator: native_operator(operator),
                inputs: inputs.clone(),
                base,
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        if !self
            .state
            .begin_field_filter_apply(request_id, target, operator, inputs)
        {
            self.worker.cancel();
            return;
        }
        self.generation = request_id;
        self.pending = PendingOperation::FieldFilterApply { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn close_field_filter(&mut self, request_id: u64) {
        let pending = matches!(self.pending,
            PendingOperation::FieldFilterDescribe { request_id: expected, .. }
            | PendingOperation::FieldFilterApply { request_id: expected }
            if expected == request_id);
        if self.state.close_field_filter(request_id) && pending {
            self.worker.cancel();
            self.state.busy = false;
        }
    }

    pub(in crate::workspace) fn publish_field_filter_description(
        &mut self,
        request_id: u64,
        description: runtime::FieldFilterDescription,
    ) {
        let PendingOperation::FieldFilterDescribe {
            request_id: expected,
            target,
        } = &self.pending
        else {
            return;
        };
        if *expected != request_id {
            return;
        }
        if native_target(target) != description.target {
            self.state.fail_field_filter(
                request_id,
                self.state.localizer.text("filtering-invalid-result"),
            );
            return;
        }
        let description = FieldFilterDescription {
            target: target.clone(),
            input_hint: view_models::input_hint(description.input_hint),
            nullable: description.nullable,
            operators: description.operators.into_iter().map(ui_operator).collect(),
            pattern_mode: description.pattern_mode.map(|mode| match mode {
                runtime::FieldFilterPatternMode::Ansi89 => FieldFilterPatternMode::Ansi89,
                runtime::FieldFilterPatternMode::Ansi92 => FieldFilterPatternMode::Ansi92,
            }),
        };
        if !self
            .state
            .set_field_filter_description(request_id, description)
        {
            self.state.fail_field_filter(
                request_id,
                self.state.localizer.text("filtering-invalid-result"),
            );
        }
    }
}

fn native_target(target: &FieldFilterTarget) -> runtime::FieldFilterTarget {
    runtime::FieldFilterTarget {
        document_revision: target.document_revision,
        table: target.table.clone(),
        column: target.column.clone(),
        filter_id: target.filter_id,
        enabled: target.enabled,
    }
}

fn native_operator(operator: FieldFilterOperator) -> NativeOperator {
    match operator {
        FieldFilterOperator::Equal => NativeOperator::Equal,
        FieldFilterOperator::NotEqual => NativeOperator::NotEqual,
        FieldFilterOperator::Less => NativeOperator::Less,
        FieldFilterOperator::LessOrEqual => NativeOperator::LessOrEqual,
        FieldFilterOperator::Greater => NativeOperator::Greater,
        FieldFilterOperator::GreaterOrEqual => NativeOperator::GreaterOrEqual,
        FieldFilterOperator::Between => NativeOperator::Between,
        FieldFilterOperator::NotBetween => NativeOperator::NotBetween,
        FieldFilterOperator::IsNull => NativeOperator::IsNull,
        FieldFilterOperator::IsNotNull => NativeOperator::IsNotNull,
        FieldFilterOperator::BeginsWith => NativeOperator::BeginsWith,
        FieldFilterOperator::NotBeginsWith => NativeOperator::NotBeginsWith,
        FieldFilterOperator::EndsWith => NativeOperator::EndsWith,
        FieldFilterOperator::NotEndsWith => NativeOperator::NotEndsWith,
        FieldFilterOperator::Contains => NativeOperator::Contains,
        FieldFilterOperator::NotContains => NativeOperator::NotContains,
    }
}

fn ui_operator(operator: NativeOperator) -> FieldFilterOperator {
    match operator {
        NativeOperator::Equal => FieldFilterOperator::Equal,
        NativeOperator::NotEqual => FieldFilterOperator::NotEqual,
        NativeOperator::Less => FieldFilterOperator::Less,
        NativeOperator::LessOrEqual => FieldFilterOperator::LessOrEqual,
        NativeOperator::Greater => FieldFilterOperator::Greater,
        NativeOperator::GreaterOrEqual => FieldFilterOperator::GreaterOrEqual,
        NativeOperator::Between => FieldFilterOperator::Between,
        NativeOperator::NotBetween => FieldFilterOperator::NotBetween,
        NativeOperator::IsNull => FieldFilterOperator::IsNull,
        NativeOperator::IsNotNull => FieldFilterOperator::IsNotNull,
        NativeOperator::BeginsWith => FieldFilterOperator::BeginsWith,
        NativeOperator::NotBeginsWith => FieldFilterOperator::NotBeginsWith,
        NativeOperator::EndsWith => FieldFilterOperator::EndsWith,
        NativeOperator::NotEndsWith => FieldFilterOperator::NotEndsWith,
        NativeOperator::Contains => FieldFilterOperator::Contains,
        NativeOperator::NotContains => FieldFilterOperator::NotContains,
    }
}

#[cfg(test)]
mod tests;
