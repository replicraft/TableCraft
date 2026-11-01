//! Borrowed table input and paid request capture before any image allocation.
use super::super::{
    CreateColumn, CreateDatabaseError, build,
    input::{Columns, Rows},
    record, request,
};
use super::layout::Layout;
use crate::{
    InspectError,
    format::{page::PAGE_BYTES, row},
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

/// One borrowed table specification for fresh creation, without admission authority.
/// All names, metadata, cells, duplicates and physical bounds are checked by the
/// operation, including tables with no rows or all-NULL non-Boolean columns.
#[derive(Clone, Copy)]
pub struct CreateTable<'a> {
    name: &'a str,
    columns: &'a [CreateColumn<'a>],
    rows: &'a [&'a [Value]],
}

impl<'a> CreateTable<'a> {
    /// Retain the caller's explicit schema and rows without cloning or coercion.
    pub const fn new(
        name: &'a str,
        columns: &'a [CreateColumn<'a>],
        rows: &'a [&'a [Value]],
    ) -> Self {
        Self {
            name,
            columns,
            rows,
        }
    }
}

pub(in crate::creation) struct Captured<'a> {
    pub(super) layout: Layout,
    pub(super) tables: Vec<request::Request<'a>>,
    pub rows: usize,
    pub columns: usize,
}

pub(in crate::creation) fn capture<'a>(
    tables: &[CreateTable<'a>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Captured<'a>, CreateDatabaseError> {
    check(budget, cancelled)?;
    let layout = Layout::capture(tables.len())?;
    budget
        .charge_allocation(
            tables.len() as u64,
            size_of::<request::Request<'_>>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut requests = Vec::new();
    requests
        .try_reserve_exact(tables.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut rows = 0_usize;
    let mut columns = 0_usize;
    let mut catalog_bytes = record::DATA_HEADER_BYTES;
    for object in build::systems() {
        catalog_bytes = catalog_bytes
            .checked_add(record::catalog_length(object.name))
            .and_then(|bytes| bytes.checked_add(row::SLOT_BYTES))
            .ok_or(CreateDatabaseError::Layout)?;
    }
    for (index, table) in tables.iter().enumerate() {
        check(budget, cancelled)?;
        for prior in &tables[..index] {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    table.name.len().max(prior.name.len()) as u64,
                )
                .map_err(InspectError::from)?;
            if table.name.eq_ignore_ascii_case(prior.name) {
                return Err(CreateDatabaseError::DuplicateTable);
            }
        }
        let request = request::validate(
            table.name,
            Columns::Values(table.columns),
            Rows::Values(table.rows),
            budget,
            cancelled,
        )?;
        rows = rows
            .checked_add(table.rows.len())
            .ok_or(CreateDatabaseError::Layout)?;
        columns = columns
            .checked_add(table.columns.len())
            .ok_or(CreateDatabaseError::Layout)?;
        catalog_bytes = catalog_bytes
            .checked_add(record::catalog_length_with_properties(
                request.catalog_name,
                request.properties.as_ref(),
            )?)
            .and_then(|bytes| bytes.checked_add(row::SLOT_BYTES))
            .ok_or(CreateDatabaseError::Layout)?;
        if catalog_bytes > PAGE_BYTES {
            return Err(CreateDatabaseError::PageCapacity);
        }
        requests.push(request);
    }
    check(budget, cancelled)?;
    Ok(Captured {
        layout,
        tables: requests,
        rows,
        columns,
    })
}
