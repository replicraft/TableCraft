//! Native admission, complete membership and one existing owned aggregate plan.
use super::snapshot::{self, Policies, SOURCE_NAME, Snapshot};
use super::{
    TotalAggregate, TotalSpec, TotalsData, TotalsDescription, TotalsError, TotalsField,
    TotalsTarget,
};
use crate::{
    Error, FilterHandle, Response,
    controls::{SCOPE, budget, check, comparison_budget},
    display::Presentation,
    document::Document,
    rows::presentation::Metadata,
};
use std::cell::RefCell;
use tablecraft_access::{NativeBrowser, NativeCollationResolver};
use tablecraft_ace::{NativeColumn, inspect_database_sort_order, inspect_table_schema};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{LegacyDateTime, Value},
};
use tablecraft_display::{NativeFormat, ScalarPresentation};
use tablecraft_query::{ExecutionOptions, QueryLimits, prepare, source::TextCollationId};
use tablecraft_schema::ScalarType;
use tablecraft_sql::{
    ParseLimits,
    query::Query,
    select::{
        Expression, FieldReference, FunctionArgument, Name, Projection, Select, Table, TableSource,
    },
};

const RETAINED_BYTES: u64 = u64::MAX;
const RETAINED_STEPS: u64 = u64::MAX;
const QUERY_ROWS: u64 = u64::MAX;
const SCAN_BATCH_ROWS: u32 = 100;
const NUMERIC: &[TotalAggregate] = &[
    TotalAggregate::Count,
    TotalAggregate::Sum,
    TotalAggregate::Average,
    TotalAggregate::Minimum,
    TotalAggregate::Maximum,
    TotalAggregate::StandardDeviation,
    TotalAggregate::Variance,
];
const DATE: &[TotalAggregate] = &[
    TotalAggregate::Count,
    TotalAggregate::Average,
    TotalAggregate::Minimum,
    TotalAggregate::Maximum,
];
const EXTENDED_DATE: &[TotalAggregate] = &[
    TotalAggregate::Count,
    TotalAggregate::Minimum,
    TotalAggregate::Maximum,
];
const COUNT: &[TotalAggregate] = &[TotalAggregate::Count];

fn available(field: &NativeColumn) -> &'static [TotalAggregate] {
    if field.complex_type_id().is_some() {
        return &[];
    }
    match field.scalar_type() {
        Some(ScalarType::DateTime) => DATE,
        Some(ScalarType::DateTimeExtended) => EXTENDED_DATE,
        Some(ScalarType::Text | ScalarType::Boolean | ScalarType::Binary | ScalarType::Guid) => {
            COUNT
        }
        Some(_) => NUMERIC,
        None => &[],
    }
}

fn copied_target(
    target: &TotalsTarget,
    output: &mut Presentation<'_>,
) -> Result<TotalsTarget, Error> {
    Ok(TotalsTarget {
        document_revision: target.document_revision,
        table: output.text(&target.table)?,
        filter_id: target.filter_id,
        enabled: target.enabled,
    })
}

impl Document {
    fn check_totals(
        &self,
        target: &TotalsTarget,
        base: Option<&FilterHandle>,
    ) -> Result<(), Error> {
        if target.document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        match (target.filter_id, base) {
            (None, None) if !target.enabled => Ok(()),
            (Some(id), Some(base)) => {
                self.check_filter(base)?;
                if base.target().filter_id != id
                    || base.target().table != target.table
                    || !base.mode_cached(target.enabled)
                {
                    return Err(TotalsError::InvalidTarget.into());
                }
                Ok(())
            }
            _ => Err(TotalsError::InvalidTarget.into()),
        }
    }

    pub(crate) fn describe_totals(
        &mut self,
        target: &TotalsTarget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if target.document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        if target.filter_id.is_none() && target.enabled {
            return Err(TotalsError::InvalidTarget.into());
        }
        let mut native = budget();
        let schema = inspect_table_schema(
            &mut self.source,
            SCOPE,
            &target.table,
            &mut native,
            cancelled,
        )?;
        let metadata = Metadata::inspect(&mut self.source, &target.table, &mut native, cancelled)?;
        let mut output = Presentation::new(cancelled);
        output.allocation(schema.columns().len(), size_of::<TotalsField>())?;
        let mut fields = Vec::new();
        fields.try_reserve_exact(schema.columns().len())?;
        for (position, field) in schema.columns().iter().enumerate() {
            let operators = available(field);
            output.allocation(operators.len(), size_of::<TotalAggregate>())?;
            let mut aggregates = Vec::new();
            aggregates.try_reserve_exact(operators.len())?;
            aggregates.extend_from_slice(operators);
            fields.push(TotalsField {
                column: metadata.column(position, field.name(), &mut output)?,
                aggregates,
            });
        }
        Ok(Response::TotalsDescription(TotalsDescription {
            target: copied_target(target, &mut output)?,
            fields,
        }))
    }

    pub(crate) fn totals(
        &mut self,
        target: &TotalsTarget,
        specs: &[TotalSpec],
        base: Option<&FilterHandle>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        self.check_totals(target, base)?;
        let mut native = budget();
        let schema = inspect_table_schema(
            &mut self.source,
            SCOPE,
            &target.table,
            &mut native,
            cancelled,
        )?;
        if specs.len() > schema.columns().len() {
            return Err(TotalsError::FieldLimit.into());
        }
        let metadata = Metadata::inspect(&mut self.source, &target.table, &mut native, cancelled)?;
        let mut retained = ResourceBudget::new(RETAINED_BYTES, 0, RETAINED_STEPS);
        retained.charge_allocation(specs.len() as u64, size_of::<usize>() as u64)?;
        let mut positions = Vec::new();
        positions.try_reserve_exact(specs.len())?;
        for spec in specs {
            let mut selected = None;
            for (position, field) in schema.columns().iter().enumerate() {
                check(cancelled)?;
                retained.charge(
                    Resource::Steps,
                    1 + field.name().len() as u64 + spec.column.len() as u64,
                )?;
                if field.name() == spec.column {
                    if !available(field).contains(&spec.aggregate) || positions.contains(&position)
                    {
                        return Err(TotalsError::UnsupportedAggregate.into());
                    }
                    selected = Some(position);
                    break;
                }
            }
            positions.push(selected.ok_or(TotalsError::UnsupportedAggregate)?);
        }
        retained.charge_allocation(positions.len() as u64, size_of::<usize>() as u64)?;
        let mut projection = Vec::new();
        projection.try_reserve_exact(positions.len())?;
        projection.extend_from_slice(&positions);
        projection.sort_unstable();
        retained.charge_allocation(projection.len() as u64, size_of::<&str>() as u64)?;
        let mut names = Vec::new();
        names.try_reserve_exact(projection.len())?;
        for &position in &projection {
            names.push(schema.columns()[position].name());
        }
        let order = inspect_database_sort_order(&mut self.source, &mut native, cancelled)?;
        let comparisons = RefCell::new(comparison_budget());
        let resolver =
            NativeCollationResolver::new_deferred_default(order, &comparisons, cancelled)?;
        let mut source = Snapshot {
            columns: Vec::new(),
            rows: Vec::new(),
            policies: Vec::new(),
            orders: Vec::new(),
        };
        retained.charge_allocation(
            projection.len() as u64,
            (size_of::<tablecraft_schema::Column>()
                + size_of::<Option<TextCollationId>>()
                + size_of::<Option<tablecraft_ace::NativeSortOrder>>()) as u64,
        )?;
        source.columns.try_reserve_exact(projection.len())?;
        source.policies.try_reserve_exact(projection.len())?;
        source.orders.try_reserve_exact(projection.len())?;
        for (slot, &position) in projection.iter().enumerate() {
            let field = &schema.columns()[position];
            retained.charge_allocation(field.name().len() as u64, 1)?;
            source.columns.push(field.column()?);
            source.orders.push(field.text_sort_order());
            source.policies.push(
                field
                    .text_sort_order()
                    .map(|_| TextCollationId::from_host(slot as u64)),
            );
        }

        let mut browser = NativeBrowser::new(&mut self.source, SCOPE, native, cancelled);
        let mut cursor = browser.projected_cursor(&target.table, &names)?;
        if cursor.columns() != source.columns.as_slice() {
            return Err(TotalsError::MembershipChanged.into());
        }
        let mut source_rows = 0_u32;
        let mut membership = 0_usize;
        let mut view_rows = 0_u32;
        loop {
            let batch = cursor.next_page(SCAN_BATCH_ROWS)?;
            if batch.offset() != source_rows {
                return Err(TotalsError::MembershipChanged.into());
            }
            for row in batch.rows() {
                check(cancelled)?;
                retained.charge(Resource::Steps, 1)?;
                let keep = match base {
                    Some(base) if target.enabled => {
                        let selected =
                            base.ordinals().get(membership).copied() == Some(source_rows);
                        if selected {
                            membership += 1;
                        }
                        selected
                    }
                    _ => true,
                };
                source_rows = source_rows
                    .checked_add(1)
                    .ok_or(TotalsError::MembershipChanged)?;
                if !keep {
                    continue;
                }
                view_rows = view_rows
                    .checked_add(1)
                    .ok_or(TotalsError::MembershipChanged)?;
                if specs.is_empty() {
                    continue;
                }
                retained.charge_allocation(row.len() as u64, size_of::<Value>() as u64)?;
                let mut copied = Vec::new();
                copied.try_reserve_exact(row.len())?;
                for value in row {
                    copied.push(snapshot::copy_value(value, &mut retained, cancelled)?);
                }
                if source.rows.len() == source.rows.capacity() {
                    let additional = source.rows.capacity().max(SCAN_BATCH_ROWS as usize);
                    retained
                        .charge_allocation(additional as u64, size_of::<Vec<Value>>() as u64)?;
                    source.rows.try_reserve_exact(additional)?;
                }
                source.rows.push(copied);
            }
            if batch.reached_end() {
                break;
            }
        }
        if base.is_some_and(|base| {
            source_rows != base.source_rows()
                || (target.enabled && membership != base.ordinals().len())
        }) {
            return Err(TotalsError::MembershipChanged.into());
        }
        check(cancelled)?;
        let mut output = Presentation::new(cancelled);
        let mut cells = output.row(specs.len())?;
        if !specs.is_empty() {
            let query = aggregate_query(specs, &mut output)?;
            let plan = prepare(&query, &source)?;
            let policies = Policies {
                source: &source,
                resolver: &resolver,
            };
            let result = plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    limits: QueryLimits {
                        bytes: RETAINED_BYTES,
                        rows: QUERY_ROWS,
                        steps: RETAINED_STEPS,
                    },
                    cancelled: Some(cancelled),
                    collation: Some(resolver.default_collation()),
                    collation_resolver: Some(&policies),
                    ..Default::default()
                },
            )?;
            let [values] = result.rows() else {
                return Err(TotalsError::MembershipChanged.into());
            };
            if values.len() != specs.len() {
                return Err(TotalsError::MembershipChanged.into());
            }
            for ((spec, &position), value) in specs.iter().zip(&positions).zip(values) {
                let converted;
                let kind = schema.columns()[position].scalar_type();
                let value = if spec.aggregate == TotalAggregate::Average
                    && kind == Some(ScalarType::DateTime)
                {
                    if let Value::Double(serial) = value {
                        converted =
                            Value::DateTime(LegacyDateTime::from_ole_days(serial.to_float())?);
                        &converted
                    } else {
                        value
                    }
                } else {
                    value
                };
                let mut cell = output.scalar(value, false)?;
                if spec.aggregate == TotalAggregate::Average
                    && kind == Some(ScalarType::Currency)
                    && let Value::Double(value) = value
                {
                    cell.presentation = Some(ScalarPresentation::CurrencyAverage {
                        bits: value.to_bits(),
                        native_format: NativeFormat::Unspecified,
                    });
                }
                metadata.format_derived(position, &spec.column, &mut cell, &mut output)?;
                cells.push(cell);
            }
        }
        output.allocation(specs.len(), size_of::<TotalSpec>())?;
        let mut copied_specs = Vec::new();
        copied_specs.try_reserve_exact(specs.len())?;
        for spec in specs {
            copied_specs.push(TotalSpec {
                column: output.text(&spec.column)?,
                aggregate: spec.aggregate,
            });
        }
        Ok(Response::Totals(TotalsData {
            target: copied_target(target, &mut output)?,
            specs: copied_specs,
            cells,
            source_rows,
            view_rows,
        }))
    }
}

fn aggregate_query(specs: &[TotalSpec], output: &mut Presentation<'_>) -> Result<Query, Error> {
    output.allocation(specs.len(), size_of::<Projection>())?;
    let mut projection = Vec::new();
    projection.try_reserve_exact(specs.len())?;
    for spec in specs {
        output.work(spec.column.len() as u64 + 1)?;
        output.allocation(
            1,
            size_of::<FunctionArgument>() + spec.column.len() + spec.aggregate.function().len(),
        )?;
        projection.push(Projection::Expression {
            expression: Expression::Call {
                qualifier: Vec::new(),
                name: Name::from_contents(spec.aggregate.function(), ParseLimits::default())?,
                arguments: vec![FunctionArgument::Expression(Expression::Field(
                    FieldReference {
                        qualifier: None,
                        name: Name::from_contents(&spec.column, ParseLimits::default())?,
                    },
                ))],
            },
            alias: None,
        });
    }
    output.allocation(1, size_of::<TableSource>() + SOURCE_NAME.len())?;
    Ok(Query::from_select(Select::from_projection(
        projection,
        vec![TableSource::Table(Table {
            database: None,
            name: Name::from_contents(SOURCE_NAME, ParseLimits::default())?,
            alias: None,
        })],
    )))
}
