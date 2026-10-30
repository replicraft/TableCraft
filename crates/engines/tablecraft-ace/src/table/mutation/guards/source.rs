use super::{
    BeforeEvent, BeforeEventGuardError, BeforeEventIntents, BeforeGuardUnsupported, buffer, error,
};
use crate::{
    EditError, InspectError, NativeImage, PayloadPresence, TableSnapshot, WriteBlocker,
    format::reader::Reader,
    header::check,
    limits::MAX_LIVE_ROWS,
    read_table,
    table::mutation::{field, preview, rows},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

mod ordinals;

pub(super) struct Frame {
    pub ordinal: usize,
    pub original: Option<Vec<Value>>,
    pub proposed: Option<Vec<Value>>,
}

pub(super) fn table(
    image: &NativeImage,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, BeforeEventGuardError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, name.len() as u64)
        .map_err(InspectError::from)?;
    preview::qualify(image).map_err(error::intent)?;
    let scope = image.catalog().catalog().scope();
    let table = read_table(&mut image.reader(), scope, name, budget, cancelled)?;
    if table.columns().is_empty() {
        return Err(BeforeEventGuardError::Unsupported(
            BeforeGuardUnsupported::SourceProfile,
        ));
    }
    for column in table.columns() {
        check(budget, cancelled)?;
        if column.data_type() != ScalarType::Int32 || column.name().eq_ignore_ascii_case("IsInsert")
        {
            return Err(BeforeEventGuardError::Unsupported(
                BeforeGuardUnsupported::SourceProfile,
            ));
        }
        lookup_work(&table, column.name(), budget, cancelled)?;
        field::fixed_column(
            table.schema(),
            column.name(),
            ScalarType::Int32,
            size_of::<i32>(),
        )
        .map_err(error::intent)?;
    }
    let object = table.schema().object_id();
    if !image
        .inspect_object_children(object, budget, cancelled)?
        .is_empty()
    {
        return Err(BeforeEventGuardError::Unsupported(
            BeforeGuardUnsupported::SourceProfile,
        ));
    }
    let payloads = image.inspect_object_payloads(object, budget, cancelled)?;
    if payloads.body() != PayloadPresence::Absent || payloads.module() != PayloadPresence::Absent {
        return Err(BeforeEventGuardError::Unsupported(
            BeforeGuardUnsupported::SourceProfile,
        ));
    }
    Ok(table)
}

fn lookup_work(
    table: &TableSnapshot,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    let mut work = 0_u64;
    for column in table.columns() {
        check(budget, cancelled)?;
        work = work
            .checked_add(column.name().len() as u64)
            .and_then(|value| value.checked_add(name.len() as u64))
            .and_then(|value| value.checked_add(1))
            .ok_or(InspectError::StructureLimit)?;
    }
    budget.charge(Resource::Steps, work)?;
    Ok(())
}

fn copy(
    values: &[Value],
    width: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Value>, EditError> {
    if values.len() != width {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let mut output = buffer::vector(width, budget, cancelled)?;
    for value in values {
        check(budget, cancelled)?;
        if !matches!(value, Value::Null | Value::Int32(_)) {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        output.push(value.clone());
    }
    Ok(output)
}

pub(super) fn frames(
    image: &NativeImage,
    table: &TableSnapshot,
    intent: BeforeEventIntents<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Frame>, BeforeEventGuardError> {
    let count = match intent {
        BeforeEventIntents::Insert(rows) => rows.len(),
        BeforeEventIntents::FixedUpdate { updates, .. } => updates.len(),
        BeforeEventIntents::Delete(rows) => rows.len(),
        BeforeEventIntents::DeleteOrdinals(rows) => rows.len(),
    };
    if count > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }
    let mut output = buffer::vector(count, budget, cancelled)?;
    let width = table.columns().len();
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };

    match intent {
        BeforeEventIntents::Insert(rows) => {
            for (ordinal, row) in rows.iter().enumerate() {
                check(reader.budget, cancelled)?;
                reader
                    .budget
                    .charge(Resource::Rows, 1)
                    .map_err(InspectError::from)?;
                let proposed =
                    copy(row.values(), width, reader.budget, cancelled).map_err(error::intent)?;
                output.push(Frame {
                    ordinal,
                    original: None,
                    proposed: Some(proposed),
                });
            }
        }
        BeforeEventIntents::FixedUpdate { column, updates } => {
            lookup_work(table, column, reader.budget, cancelled)?;
            let selected =
                field::fixed_column(table.schema(), column, ScalarType::Int32, size_of::<i32>())
                    .map_err(error::intent)?;
            rows::unique(updates, reader.budget, cancelled).map_err(error::intent)?;
            rows::visit_with_budget(&mut reader, table.schema(), updates, |budget, _, update| {
                let original = table.rows().get(update.row()).ok_or(EditError::Conflict)?;
                if original.get(selected) != Some(update.expected())
                    || !matches!(update.value(), Value::Null | Value::Int32(_))
                {
                    return Err(EditError::Conflict);
                }
                let old = copy(original, width, budget, cancelled)?;
                let mut proposed = copy(original, width, budget, cancelled)?;
                proposed[selected] = update.value().clone();
                output.push(Frame {
                    ordinal: update.row(),
                    original: Some(old),
                    proposed: Some(proposed),
                });
                Ok(())
            })
            .map_err(error::intent)?;
        }
        BeforeEventIntents::Delete(deletes) => {
            rows::unique(deletes, reader.budget, cancelled).map_err(error::intent)?;
            rows::visit_with_budget(
                &mut reader,
                table.schema(),
                deletes,
                |budget, row, delete| {
                    budget
                        .charge(Resource::Steps, row.data.len() as u64)
                        .map_err(InspectError::from)?;
                    if row.data != delete.expected_bytes() {
                        return Err(EditError::Conflict);
                    }
                    let original = table.rows().get(delete.row()).ok_or(EditError::Conflict)?;
                    let old = copy(original, width, budget, cancelled)?;
                    output.push(Frame {
                        ordinal: delete.row(),
                        original: Some(old),
                        proposed: None,
                    });
                    Ok(())
                },
            )
            .map_err(error::intent)?;
        }
        BeforeEventIntents::DeleteOrdinals(ordinals) => {
            ordinals::append(&mut reader, table, ordinals, &mut output)?;
        }
    }
    check(reader.budget, cancelled)?;
    Ok(output)
}

pub(super) fn event_name(event: BeforeEvent) -> &'static str {
    match event {
        BeforeEvent::Insert | BeforeEvent::Update => "BeforeChange",
        BeforeEvent::Delete => "BeforeDelete",
    }
}
