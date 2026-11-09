//! Native view capabilities stay retained; protocol IDs never manufacture row maps.
use super::{Session, arguments as arg};
use serde_json::{Value, json};
use tablecraft_runtime::{
    Command, FieldFilterOperator, FieldFilterTarget, FilterComparison, FilterDefinition,
    FilterHandle, FilterSelectionTarget, Response, SortSpec, SortTarget, TotalAggregate, TotalSpec,
    TotalsTarget,
};

#[derive(Default)]
pub(super) struct ViewState {
    handle: Option<(u64, FilterHandle)>,
    definition: Option<(u64, FilterDefinition)>,
}

impl ViewState {
    pub(super) fn retire_native(&mut self) {
        self.handle = None;
    }
}

impl Session {
    pub(super) fn view_handle(
        &self,
        args: &Value,
        revision: u64,
        table: Option<&str>,
    ) -> Result<Option<FilterHandle>, String> {
        let Some(value) = arg::optional(args, "filter_id") else {
            return Ok(None);
        };
        let requested = value.as_u64().ok_or("filter_id must be an integer")?;
        let Some((_, handle)) = self.view_state.handle.as_ref().filter(|(id, handle)| {
            *id == requested
                && handle.target().document_revision == revision
                && table.is_none_or(|table| table == handle.target().table)
        }) else {
            return Err("Unknown, superseded, wrong-table or stale filter handle".into());
        };
        // NO_CLONE: FilterHandle shares its private immutable native row map. The host
        // retains the actual handle if a worker command fails; the command must own
        // another thread-safe handle, and its public API cannot borrow across threads.
        Ok(Some(handle.clone()))
    }

    pub(super) fn view_operation(
        &mut self,
        operation: &str,
        args: &Value,
    ) -> Result<Value, String> {
        let revision = self.current_revision(args)?;
        let command = match operation {
            "filter_page" | "clear_filter" => {
                let handle = self
                    .view_handle(args, revision, None)?
                    .ok_or("filter_id is required")?;
                if operation == "clear_filter" {
                    Command::ClearFilter { handle }
                } else {
                    Command::FilterPage {
                        handle,
                        offset: arg::u32_value(args, "offset")?,
                        enabled: arg::boolean(args, "enabled", true)?,
                    }
                }
            }
            "reapply_filter" => {
                let requested = arg::u64_value(args, "definition_id")?;
                let Some((_, definition)) = self
                    .view_state
                    .definition
                    .as_ref()
                    .filter(|(id, _)| *id == requested)
                else {
                    return Err("Unknown or superseded inert filter definition".into());
                };
                // NO_CLONE: The inert definition shares immutable criteria and survives a
                // refused fresh-source revalidation; the worker independently owns the
                // same bounded definition while it rebuilds an actual native handle.
                let definition = definition.clone();
                Command::ReapplyFilter {
                    definition,
                    document_revision: revision,
                    enabled: arg::boolean(args, "enabled", true)?,
                    offset: arg::u32_value(args, "offset")?,
                }
            }
            _ => {
                let table = arg::owned(args, "table")?;
                let base = self.view_handle(args, revision, Some(&table))?;
                let filter_id = base.as_ref().map(|handle| handle.target().filter_id);
                let enabled = arg::boolean(args, "enabled", base.is_some())?;
                match operation {
                    "describe_field_filter" => Command::DescribeFieldFilter {
                        target: FieldFilterTarget {
                            document_revision: revision,
                            table,
                            column: arg::owned(args, "column")?,
                            filter_id,
                            enabled,
                        },
                    },
                    "apply_field_filter" => {
                        let target = FieldFilterTarget {
                            document_revision: revision,
                            table,
                            column: arg::owned(args, "column")?,
                            filter_id,
                            enabled,
                        };
                        let inputs =
                            args.get("inputs")
                                .and_then(Value::as_array)
                                .ok_or("inputs must be an array")?
                                .iter()
                                .map(|value| {
                                    value.as_str().map(String::from).ok_or_else(|| {
                                        "Each filter input must be literal text".into()
                                    })
                                })
                                .collect::<Result<Vec<String>, String>>()?;
                        Command::ApplyFieldFilter {
                            target,
                            operator: filter_operator(arg::text(args, "operator")?)?,
                            inputs,
                            base,
                        }
                    }
                    "filter_selection" => {
                        let comparison = match arg::text(args, "comparison")? {
                            "equal" => FilterComparison::Equal,
                            "not_equal" => FilterComparison::NotEqual,
                            _ => return Err("comparison must be equal or not_equal".into()),
                        };
                        Command::FilterSelection {
                            target: FilterSelectionTarget {
                                document_revision: revision,
                                table,
                                parent_ordinal: arg::u32_value(args, "parent_ordinal")?,
                                column: arg::owned(args, "column")?,
                                filter_id,
                                enabled,
                            },
                            comparison,
                            base,
                        }
                    }
                    "sort_view" => {
                        let sort = args
                            .get("sort")
                            .filter(|value| !value.is_null())
                            .map(|value| {
                                Ok::<_, String>(SortSpec {
                                    column: arg::owned(value, "column")?,
                                    descending: arg::boolean(value, "descending", false)?,
                                })
                            })
                            .transpose()?;
                        Command::SortView {
                            target: SortTarget {
                                document_revision: revision,
                                table,
                                filter_id,
                            },
                            sort,
                            base,
                            enabled,
                        }
                    }
                    "describe_totals" => Command::DescribeTotals {
                        target: TotalsTarget {
                            document_revision: revision,
                            table,
                            filter_id,
                            enabled,
                        },
                    },
                    "calculate_totals" => {
                        let values = args
                            .get("specs")
                            .and_then(Value::as_array)
                            .ok_or("specs must be an array")?;
                        if values.len() > tablecraft_runtime::MAX_TOTAL_FIELDS {
                            return Err("Too many total fields".into());
                        }
                        let specs = values
                            .iter()
                            .map(|value| {
                                Ok(TotalSpec {
                                    column: arg::owned(value, "column")?,
                                    aggregate: total_aggregate(arg::text(value, "aggregate")?)?,
                                })
                            })
                            .collect::<Result<Vec<_>, String>>()?;
                        Command::Totals {
                            target: TotalsTarget {
                                document_revision: revision,
                                table,
                                filter_id,
                                enabled,
                            },
                            specs,
                            base,
                        }
                    }
                    _ => return Err("Unknown native view operation".into()),
                }
            }
        };
        let response = self.request(command)?;
        match response {
            Response::FilteredData(data) => {
                let id = self.next_handle()?;
                let definition_id = self.next_handle()?;
                let result = json!({"filter_id": id, "definition_id": definition_id, "document_revision": data.target.document_revision, "table": data.target.table, "enabled": data.enabled, "offset": data.offset, "columns": super::response::columns(&data.columns), "rows": super::response::rows(&data.rows), "source_ordinals": data.source_ordinals, "matched_rows": data.matched_rows, "source_rows": data.source_rows, "has_more": data.has_more, "sort": data.sort.as_ref().map(|sort| json!({"column": sort.column, "descending": sort.descending})), "filter_count": data.filter_count});
                self.view_state.definition = Some((definition_id, data.handle.definition()));
                self.view_state.handle = Some((id, data.handle));
                Ok(result)
            }
            Response::FieldFilterDescription(description) => Ok(
                json!({"document_revision": description.target.document_revision, "table": description.target.table, "column": description.target.column, "input_hint": format!("{:?}", description.input_hint), "nullable": description.nullable, "operators": description.operators.into_iter().map(filter_operator_name).collect::<Vec<_>>(), "pattern_mode": description.pattern_mode.map(|mode| format!("{mode:?}"))}),
            ),
            Response::TotalsDescription(description) => Ok(
                json!({"document_revision": description.target.document_revision, "table": description.target.table, "enabled": description.target.enabled, "fields": description.fields.into_iter().map(|field| json!({"column": field.column.name, "aggregates": field.aggregates.into_iter().map(total_aggregate_name).collect::<Vec<_>>()})).collect::<Vec<_>>()}),
            ),
            Response::Totals(data) => Ok(
                json!({"document_revision": data.target.document_revision, "table": data.target.table, "enabled": data.target.enabled, "specs": data.specs.iter().map(|spec| json!({"column": spec.column, "aggregate": total_aggregate_name(spec.aggregate)})).collect::<Vec<_>>(), "cells": data.cells.iter().map(super::response::cell).collect::<Vec<_>>(), "source_rows": data.source_rows, "view_rows": data.view_rows}),
            ),
            response => self.response(response),
        }
    }
}

fn filter_operator(value: &str) -> Result<FieldFilterOperator, String> {
    [
        FieldFilterOperator::Equal,
        FieldFilterOperator::NotEqual,
        FieldFilterOperator::Less,
        FieldFilterOperator::LessOrEqual,
        FieldFilterOperator::Greater,
        FieldFilterOperator::GreaterOrEqual,
        FieldFilterOperator::Between,
        FieldFilterOperator::NotBetween,
        FieldFilterOperator::IsNull,
        FieldFilterOperator::IsNotNull,
        FieldFilterOperator::BeginsWith,
        FieldFilterOperator::NotBeginsWith,
        FieldFilterOperator::EndsWith,
        FieldFilterOperator::NotEndsWith,
        FieldFilterOperator::Contains,
        FieldFilterOperator::NotContains,
    ]
    .into_iter()
    .find(|operator| filter_operator_name(*operator) == value)
    .ok_or_else(|| "Unsupported filter operator".into())
}

fn filter_operator_name(value: FieldFilterOperator) -> &'static str {
    match value {
        FieldFilterOperator::Equal => "equal",
        FieldFilterOperator::NotEqual => "not_equal",
        FieldFilterOperator::Less => "less",
        FieldFilterOperator::LessOrEqual => "less_or_equal",
        FieldFilterOperator::Greater => "greater",
        FieldFilterOperator::GreaterOrEqual => "greater_or_equal",
        FieldFilterOperator::Between => "between",
        FieldFilterOperator::NotBetween => "not_between",
        FieldFilterOperator::IsNull => "is_null",
        FieldFilterOperator::IsNotNull => "is_not_null",
        FieldFilterOperator::BeginsWith => "begins_with",
        FieldFilterOperator::NotBeginsWith => "not_begins_with",
        FieldFilterOperator::EndsWith => "ends_with",
        FieldFilterOperator::NotEndsWith => "not_ends_with",
        FieldFilterOperator::Contains => "contains",
        FieldFilterOperator::NotContains => "not_contains",
    }
}

fn total_aggregate(value: &str) -> Result<TotalAggregate, String> {
    [
        TotalAggregate::Count,
        TotalAggregate::Sum,
        TotalAggregate::Average,
        TotalAggregate::Minimum,
        TotalAggregate::Maximum,
        TotalAggregate::StandardDeviation,
        TotalAggregate::Variance,
    ]
    .into_iter()
    .find(|aggregate| total_aggregate_name(*aggregate) == value)
    .ok_or_else(|| "Unsupported total aggregate".into())
}

fn total_aggregate_name(value: TotalAggregate) -> &'static str {
    match value {
        TotalAggregate::Count => "count",
        TotalAggregate::Sum => "sum",
        TotalAggregate::Average => "average",
        TotalAggregate::Minimum => "minimum",
        TotalAggregate::Maximum => "maximum",
        TotalAggregate::StandardDeviation => "standard_deviation",
        TotalAggregate::Variance => "variance",
    }
}
