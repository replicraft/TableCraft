//! Metadata-only binding and explicit, bounded complete stored-data scans.
use std::cell::{OnceCell, RefCell};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_formats::{
    ReadError,
    template::{TemplateError, TemplateObject, TemplatePackage},
};
use tablecraft_model::ObjectKind;
use tablecraft_query::{QueryError, Source};
use tablecraft_schema::Column;
use tablecraft_sql::query::Query;

struct Table {
    object: TemplateObject,
    columns: OnceCell<Result<Vec<Column>, TemplateError>>,
}
/// Read-only query bridge for inline template data. Binding parses only a
/// selected inline XSD prefix; it never decodes records or executes object bodies.
/// Scans validate complete table snapshots before callbacks. Shared host budgets
/// apply across repeated scans; the immutable package supplies stable input.
pub struct TemplateSource<'a> {
    package: &'a TemplatePackage,
    tables: Vec<Table>,
    queries: Vec<String>,
    budget: RefCell<ResourceBudget>,
    cancelled: &'a dyn Fn() -> bool,
}
impl<'a> TemplateSource<'a> {
    /// Discover object identities only. Missing/linked/unsupported table schemas
    /// fail when selected; unrelated application bodies are not interpreted.
    pub fn new(
        package: &'a TemplatePackage,
        mut budget: ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, TemplateError> {
        let objects = package.objects(&mut budget, cancelled)?;
        let queries: Vec<_> = objects
            .iter()
            .filter(|object| object.kind() == ObjectKind::Query)
            .map(|object| object.name().to_owned())
            .collect();
        if objects.iter().any(|object| {
            object.kind() == ObjectKind::Table
                && queries
                    .iter()
                    .any(|query| query.eq_ignore_ascii_case(object.name()))
        }) {
            return Err(TemplateError::Malformed);
        }
        let tables = objects
            .into_iter()
            .filter(|object| object.kind() == ObjectKind::Table)
            .map(|object| Table {
                object,
                columns: OnceCell::new(),
            })
            .collect();
        Ok(Self {
            package,
            tables,
            queries,
            budget: RefCell::new(budget),
            cancelled,
        })
    }
    fn table(&self, name: &str) -> Result<&Table, QueryError> {
        self.tables
            .iter()
            .find(|table| table.object.name().eq_ignore_ascii_case(name))
            .ok_or(QueryError::UnknownName)
    }
}
impl Source for TemplateSource<'_> {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        if self
            .queries
            .iter()
            .any(|query| query.eq_ignore_ascii_case(name))
        {
            return Err(QueryError::Unsupported);
        }
        Ok(None)
    }
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        let table = self.table(table)?;
        {
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            check(&mut budget, self.cancelled).map_err(|error| query_error(&error))?;
            if table.columns.get().is_none() {
                let result =
                    self.package
                        .columns_for_object(&table.object, &mut budget, self.cancelled);
                if let Err(error @ TemplateError::Read(_)) = &result {
                    return Err(query_error(error));
                }
                table.columns.set(result).map_err(|_| QueryError::Source)?;
            }
        }
        match table.columns.get().ok_or(QueryError::Source)? {
            Ok(columns) => Ok(columns),
            Err(error) => Err(query_error(error)),
        }
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        let table = self.table(table)?;
        let snapshot = {
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            self.package
                .read_table(table.object.name(), &mut budget, self.cancelled)
                .map_err(|error| query_error(&error))?
        };
        if snapshot.columns() != self.columns(table.object.name())? {
            return Err(QueryError::SchemaChanged);
        }
        for row in snapshot.rows() {
            {
                let mut budget = self
                    .budget
                    .try_borrow_mut()
                    .map_err(|_| QueryError::Source)?;
                check(&mut budget, self.cancelled).map_err(|error| query_error(&error))?;
            }
            visitor(row)?;
        }
        Ok(())
    }
}
fn query_error(error: &TemplateError) -> QueryError {
    match error {
        TemplateError::Read(ReadError::Cancelled) => QueryError::Cancelled,
        TemplateError::Read(ReadError::Limit(limit)) => QueryError::Limit(*limit),
        TemplateError::Read(ReadError::Allocation) => QueryError::Allocation,
        TemplateError::Unsupported => QueryError::Unsupported,
        _ => QueryError::Source,
    }
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), TemplateError> {
    if cancelled() {
        return Err(ReadError::Cancelled.into());
    }
    budget
        .charge(tablecraft_core::limits::Resource::Steps, 1)
        .map_err(ReadError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests;
