//! Issued SQL syntax selects existing consuming schema reviews, never direct writes.
use super::{EditError, platform};
use crate::{
    Error, InspectedQuery, QueryOperation, Response,
    controls::{action_query_budget, check},
    document::Document,
};
use tablecraft_core::limits::Resource;
use tablecraft_sql::{
    ParseLimits, parse_query,
    query::{
        Statement,
        definition::{Definition, TableElement},
    },
};

impl Document {
    pub(crate) fn review_schema_sql(
        &mut self,
        target: &InspectedQuery,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        self.require_query_inspection(target, cancelled)?;
        if target.operation() != QueryOperation::Schema {
            return Err(Error::SchemaSqlUnsupported);
        }
        if !self.source.editable_native() {
            return Err(EditError::ReadOnlySource.into());
        }
        let mut work = action_query_budget();
        work.charge(Resource::Bytes, target.sql().len() as u64)?;
        work.charge(Resource::Steps, target.sql().len() as u64)?;
        let parsed = parse_query(target.sql(), ParseLimits::default())?;
        if !parsed.query().parameters.is_empty() {
            return Err(Error::SchemaSqlUnsupported);
        }
        let Statement::Definition(definition) = &parsed.query().statement else {
            return Err(Error::SchemaSqlUnsupported);
        };
        if let Definition::CreateTable(table) = definition.as_ref()
            && (table.temporary || table.elements.iter().any(|element| {
                matches!(element, TableElement::Column(column) if column.default_value.is_some())
            }))
        {
            // Temporary-session storage is not this durable service. DEFAULT
            // is OLE DB/ADO syntax and remains unavailable in SQL View.
            return Err(Error::SchemaSqlUnsupported);
        }
        check(cancelled)?;
        platform::schema_sql::review(self, definition, cancelled)
    }
}
