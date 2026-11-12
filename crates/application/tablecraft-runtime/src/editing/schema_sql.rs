//! Issued SQL syntax selects existing consuming schema reviews, never direct writes.
use super::{EditError, platform};
use crate::{
    Error, InspectedQuery, QueryOperation, Response,
    controls::{action_query_budget, check},
    document::Document,
    query::parameters::ParameterInput,
};
use tablecraft_core::limits::Resource;
use tablecraft_expressions::functions::dates::clock::Clock;
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
        _clock: &dyn Clock,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        self.review_schema_sql_with_inputs(target, None, _clock, cancelled)
    }

    pub(crate) fn review_parameterized_schema_sql(
        &mut self,
        target: &InspectedQuery,
        parameters: &[ParameterInput],
        clock: &dyn Clock,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        self.review_schema_sql_with_inputs(target, Some(parameters), clock, cancelled)
    }

    fn review_schema_sql_with_inputs(
        &mut self,
        target: &InspectedQuery,
        parameters: Option<&[ParameterInput]>,
        _clock: &dyn Clock,
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
        if parameters.is_none() && !parsed.query().parameters.is_empty() {
            return Err(Error::SchemaSqlUnsupported);
        }
        if matches!(&parsed.query().statement, Statement::MakeTable(_)) {
            #[cfg(feature = "ace16-create")]
            return platform::schema_sql::review_make_table(
                self,
                parsed.into_query(),
                parameters.unwrap_or(&[]),
                _clock,
                cancelled,
            );
            #[cfg(not(feature = "ace16-create"))]
            return Err(Error::SchemaSqlUnsupported);
        }
        if !parsed.query().parameters.is_empty()
            || parameters.is_some_and(|inputs| !inputs.is_empty())
        {
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
