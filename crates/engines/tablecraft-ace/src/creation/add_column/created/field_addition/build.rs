//! Reuse native declarations, MR2 serialization and both property placement owners.
use super::{FieldAdditionError as Error, FieldAdditionTarget, verify};
use crate::{
    CreateColumn, InspectError, NativeImage, TableCheck,
    creation::add_column::property_preservation,
    creation::{
        build,
        input::{Columns, Rows},
        properties::{self, Order},
        request,
    },
    format::page::PAGE_BYTES,
    header::check,
    table::mutation::catalog_property,
};
use tablecraft_core::limits::ResourceBudget;

const INLINE_DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();

fn presence(
    columns: &[CreateColumn<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(bool, bool, bool, bool), Error> {
    let mut required = false;
    let mut allow_empty = false;
    let mut default_value = false;
    let mut description = false;
    for field in columns {
        check(budget, cancelled)?;
        required |= field.required().is_some();
        allow_empty |= field.allow_zero_length().is_some();
        default_value |= field.default_literal().is_some();
        description |= field.description().is_some();
    }
    Ok((required, allow_empty, default_value, description))
}

pub(super) fn initial_order(
    columns: &[CreateColumn<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Order, Error> {
    let (required, allow_empty, default_value, description) = presence(columns, budget, cancelled)?;
    Ok(Order::empty().extend(required, allow_empty, default_value, description, false)?)
}

pub(super) fn extend_order(
    order: Order,
    columns: &[CreateColumn<'_>],
    target: FieldAdditionTarget,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Order, Error> {
    let (required, allow_empty, default_value, description) = presence(columns, budget, cancelled)?;
    Ok(order.extend(
        required,
        allow_empty,
        default_value,
        description,
        target == FieldAdditionTarget::NumberLongZero,
    )?)
}

/// Rebuild a canonical seed from typed declarations and known dictionary order.
/// No original raw bytes are copied or normalized into the expected source.
pub(super) fn canonical(
    table: &str,
    columns: &[CreateColumn<'_>],
    order: Order,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    let mut request = request::validate(
        table,
        Columns::Values(columns),
        Rows::Values(&[]),
        budget,
        cancelled,
    )?;
    if let Some(prepared) = request.properties.take() {
        request.properties = Some(prepared.with_order(order)?);
    } else if !order.names().is_empty() {
        return Err(Error::Source);
    }
    Ok(build::build(&request, budget, cancelled)?)
}

fn property_body(
    columns: &[CreateColumn<'_>],
    order: Order,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    let prepared = properties::capture(
        Columns::Values(columns),
        Rows::Values(&[]),
        budget,
        cancelled,
    )?
    .ok_or(Error::Preservation)?
    .with_order(order)?;
    let length = prepared.inline_length();
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    bytes.resize(length, 0);
    prepared.write_inline(&mut bytes, budget, cancelled)?;
    bytes.drain(..INLINE_DESCRIPTOR_BYTES);
    Ok(bytes)
}

pub(super) struct Addition<'a, 'columns> {
    pub checked: &'a TableCheck,
    pub columns: &'a [CreateColumn<'columns>],
    pub order: Order,
    pub external: bool,
    pub target: FieldAdditionTarget,
}

pub(super) fn append(
    before: &NativeImage,
    addition: Addition<'_, '_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    check(budget, cancelled)?;
    let Addition {
        checked,
        columns,
        order,
        external,
        target,
    } = addition;
    let column = columns.last().ok_or(Error::Source)?.name();
    let rebuilt = canonical(checked.schema().name(), columns, order, budget, cancelled)?;
    let mut after = super::super::super::common::copy_definition(
        before,
        &rebuilt,
        checked.schema().definition_page() as usize * PAGE_BYTES,
        budget,
        cancelled,
    )?;
    verify::definition(before, &after, checked, column, target, budget, cancelled)?;
    let body = property_body(columns, order, budget, cancelled)?;
    if external {
        let change =
            catalog_property::compose_repeated(before, checked.schema(), body, budget, cancelled)?;
        change.apply(&mut after, budget, cancelled)?;
        property_preservation::repeated(
            before, &after, checked, column, &change, budget, cancelled,
        )?;
    } else {
        let (candidate, change) = super::super::super::initial_property::apply(
            before,
            checked.schema(),
            after,
            body,
            budget,
            cancelled,
        )?;
        after = candidate;
        property_preservation::initial(
            before, &after, checked, column, &change, budget, cancelled,
        )?;
    }
    Ok(after)
}
