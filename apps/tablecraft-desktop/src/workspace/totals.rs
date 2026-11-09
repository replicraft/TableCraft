//! Forward derived totals requests while keeping native capabilities host-owned.
use super::{Desktop, PendingOperation};
use crate::view_models;
use crate::workspace::request_read;
use tablecraft_runtime::{self as runtime, Command, FilterHandle};
use tablecraft_ui::{
    Column, TotalAggregate, TotalSpec, TotalsData, TotalsDescription, TotalsField, TotalsTarget,
};

impl Desktop {
    fn totals_base(&self, target: &TotalsTarget) -> Result<Option<FilterHandle>, ()> {
        let Some(filter_id) = target.filter_id else {
            return Ok(None);
        };
        self.filter_handle(target.document_revision, &target.table, filter_id)
            .map(Some)
            .ok_or(())
    }

    pub(super) fn describe_totals(&mut self, target: TotalsTarget) {
        if self.editing.has_save() || !self.state.can_describe_totals(&target) {
            return;
        }
        if self.totals_base(&target).is_err() {
            self.state
                .reject_totals_request(&target, self.state.localizer.text("totals-stale"));
            return;
        }

        let Some(request_id) = request_read(
            Command::DescribeTotals {
                target: native_target(&target),
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        if !self.state.begin_totals_description(&target, request_id) {
            self.worker.cancel();
            return;
        }
        self.generation = request_id;
        self.pending = PendingOperation::TotalsDescribe { request_id, target };
        self.state.busy = true;
    }

    pub(super) fn compute_totals(&mut self, target: TotalsTarget, specs: Vec<TotalSpec>) {
        if self.editing.has_save() || !self.state.can_compute_totals(&target, &specs) {
            return;
        }
        let Ok(base) = self.totals_base(&target) else {
            self.state
                .reject_totals_request(&target, self.state.localizer.text("totals-stale"));
            return;
        };
        let Some(request_id) = request_read(
            Command::Totals {
                target: native_target(&target),
                specs: specs.iter().map(native_spec).collect(),
                base,
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        // Shared totals pending retains exact requested specifications, and host pending
        // separately validates native returned specs before publication.
        if !self
            .state
            .begin_totals_compute(request_id, &target, specs.clone())
        {
            self.worker.cancel();
            return;
        }
        self.generation = request_id;
        self.pending = PendingOperation::TotalsCompute {
            request_id,
            target,
            specs,
        };
        self.state.busy = true;
    }

    pub(super) fn cancel_totals(&mut self, request_id: u64) {
        let owned = matches!(
            self.pending,
            PendingOperation::TotalsDescribe { request_id: expected, .. }
                | PendingOperation::TotalsCompute { request_id: expected, .. }
                if expected == request_id
        );
        if self.state.cancel_totals(request_id) && owned {
            self.worker.cancel();
            self.state.busy = false;
        }
    }

    pub(super) fn publish_totals_description(
        &mut self,
        request_id: u64,
        description: runtime::TotalsDescription,
    ) {
        let PendingOperation::TotalsDescribe {
            request_id: expected,
            target,
        } = &self.pending
        else {
            return;
        };
        if *expected != request_id {
            return;
        }

        let description = TotalsDescription {
            target: ui_target(description.target),
            fields: description
                .fields
                .into_iter()
                .map(|field| TotalsField {
                    column: Column {
                        name: field.column.name,
                        caption: field.column.caption,
                        native_number: field.column.native_number,
                    },
                    aggregates: field.aggregates.into_iter().map(ui_aggregate).collect(),
                })
                .collect(),
        };
        if description.target != *target
            || !self.state.set_totals_description(request_id, description)
        {
            self.state.fail_totals(
                request_id,
                self.state.localizer.text("totals-invalid-result"),
            );
        }
    }

    pub(super) fn publish_totals(&mut self, request_id: u64, result: runtime::TotalsData) {
        let PendingOperation::TotalsCompute {
            request_id: expected,
            target,
            specs,
        } = &self.pending
        else {
            return;
        };
        if *expected != request_id {
            return;
        }

        let result = TotalsData {
            target: ui_target(result.target),
            specs: result
                .specs
                .into_iter()
                .map(|spec| TotalSpec {
                    column: spec.column,
                    aggregate: ui_aggregate(spec.aggregate),
                })
                .collect(),
            cells: result.cells.into_iter().map(view_models::cell).collect(),
            source_rows: result.source_rows,
            view_rows: result.view_rows,
        };
        if result.target != *target
            || result.specs != *specs
            || !self.state.set_totals_result(request_id, result)
        {
            self.state.fail_totals(
                request_id,
                self.state.localizer.text("totals-invalid-result"),
            );
        }
    }
}

fn native_target(target: &TotalsTarget) -> runtime::TotalsTarget {
    runtime::TotalsTarget {
        document_revision: target.document_revision,
        // Runtime totals target and host/UI totals witnesses each outlive dispatch; they need
        // separately owned exact source table identities.
        table: target.table.clone(),
        filter_id: target.filter_id,
        enabled: target.enabled,
    }
}

fn ui_target(target: runtime::TotalsTarget) -> TotalsTarget {
    TotalsTarget {
        document_revision: target.document_revision,
        table: target.table,
        filter_id: target.filter_id,
        enabled: target.enabled,
    }
}

fn native_spec(spec: &TotalSpec) -> runtime::TotalSpec {
    runtime::TotalSpec {
        // Runtime aggregate spec and shared/host pending aggregate specs each retain the
        // requested column until exact result comparison.
        column: spec.column.clone(),
        aggregate: match spec.aggregate {
            TotalAggregate::Count => runtime::TotalAggregate::Count,
            TotalAggregate::Sum => runtime::TotalAggregate::Sum,
            TotalAggregate::Average => runtime::TotalAggregate::Average,
            TotalAggregate::Minimum => runtime::TotalAggregate::Minimum,
            TotalAggregate::Maximum => runtime::TotalAggregate::Maximum,
            TotalAggregate::StandardDeviation => runtime::TotalAggregate::StandardDeviation,
            TotalAggregate::Variance => runtime::TotalAggregate::Variance,
        },
    }
}

fn ui_aggregate(aggregate: runtime::TotalAggregate) -> TotalAggregate {
    match aggregate {
        runtime::TotalAggregate::Count => TotalAggregate::Count,
        runtime::TotalAggregate::Sum => TotalAggregate::Sum,
        runtime::TotalAggregate::Average => TotalAggregate::Average,
        runtime::TotalAggregate::Minimum => TotalAggregate::Minimum,
        runtime::TotalAggregate::Maximum => TotalAggregate::Maximum,
        runtime::TotalAggregate::StandardDeviation => TotalAggregate::StandardDeviation,
        runtime::TotalAggregate::Variance => TotalAggregate::Variance,
    }
}

#[cfg(test)]
mod tests;
