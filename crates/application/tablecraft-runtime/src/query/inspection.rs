//! Issued SQL snapshots bind declaration prompts to one exact opened document.
use super::{
    SQL_BYTES, execute,
    parameters::{self, ParameterInput},
};
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check, comparison_budget},
    display::Presentation,
    document::Document,
};
use std::{cell::RefCell, sync::Arc};
use tablecraft_access::NativeSource;
use tablecraft_expressions::functions::dates::clock::Clock;
use tablecraft_sql::{ParseLimits, parse_query};

// Charge strong/weak reference counters in addition to the retained target.
// This accounting does not model allocator-specific metadata.
const ARC_COUNTERS: usize = 2;

/// Inert SQL captured by runtime inspection. Fields have no public constructors
/// or mutation access. This grants no permission to mutate a database or bypass
/// execution admission. Debug/Serde are omitted to avoid implicit SQL logging.
pub struct InspectedQuery {
    sql: String,
    document_revision: u64,
    source: Arc<()>,
}

impl InspectedQuery {
    pub fn sql(&self) -> &str {
        &self.sql
    }

    pub fn document_revision(&self) -> u64 {
        self.document_revision
    }
}

impl Document {
    /// Validate an issued inspection before any consumer parses or captures data.
    pub(crate) fn require_query_inspection(
        &self,
        target: &InspectedQuery,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Error> {
        check(cancelled)?;
        if target.document_revision != self.revision
            || !Arc::ptr_eq(&target.source, &self.filter_source)
        {
            return Err(Error::StaleQuery);
        }
        Ok(())
    }

    /// Parse only syntax and declared parameter types. No native source callback,
    /// binding scan, execution provider, clock, or random sequence is consulted.
    pub(crate) fn inspect_query(
        &self,
        document_revision: u64,
        sql: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if document_revision != self.revision {
            return Err(Error::StaleQuery);
        }
        if sql.len() > SQL_BYTES {
            return Err(Error::SqlLimit);
        }
        let parsed = parse_query(sql, ParseLimits::default())?;
        let mut output = Presentation::new(cancelled);
        let parameters = parameters::specs(parsed.query(), &mut output)?;
        output.allocation(
            1,
            size_of::<InspectedQuery>() + ARC_COUNTERS * size_of::<usize>(),
        )?;
        let target = Arc::new(InspectedQuery {
            sql: output.text(sql)?,
            document_revision,
            source: Arc::clone(&self.filter_source),
        });
        check(cancelled)?;
        Ok(Response::QueryParameters { target, parameters })
    }

    /// Recheck the issued source identity before processing any supplied input.
    /// Literal binding remains separate from SQL evaluation and display syntax.
    pub(crate) fn run_query(
        &mut self,
        target: &InspectedQuery,
        inputs: &[ParameterInput],
        clock: &dyn Clock,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        self.require_query_inspection(target, cancelled)?;
        let mut output = Presentation::new(cancelled);
        parameters::validate_input_limits(inputs, &mut output)?;
        let parsed = parse_query(target.sql(), ParseLimits::default())?;
        let bindings = parameters::bindings(parsed.query(), inputs, &mut output)?;
        check(cancelled)?;
        let source = NativeSource::new(&mut self.source, SCOPE, budget(), cancelled)?;
        let comparison_pool = RefCell::new(comparison_budget());
        let (mut source, resolver) =
            source.with_deferred_database_collation_resolver(&comparison_pool)?;
        execute(
            parsed.query(),
            &mut source,
            &resolver,
            &bindings,
            "Query results",
            clock,
            cancelled,
        )
    }
}
