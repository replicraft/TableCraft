//! Issued SQL snapshots bind declaration prompts to one exact opened document.
use super::{
    RunSettings, SQL_BYTES, execute,
    parameters::{self, ParameterInput},
};
use crate::{
    Error, QueryRunOptions, Response,
    controls::{SCOPE, check},
    display::Presentation,
    document::Document,
};
use std::{cell::RefCell, sync::Arc};
use tablecraft_access::NativeSource;
use tablecraft_expressions::functions::dates::clock::Clock;
use tablecraft_sql::{ParseLimits, parse_query, query::Statement};

// Charge strong/weak reference counters in addition to the retained target.
// This accounting does not model allocator-specific metadata.
const ARC_COUNTERS: usize = 2;

/// Syntax-only dispatch class for an inspected query. This grants no binding,
/// native write, source discovery or execution authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryOperation {
    Read,
    Update,
    Delete,
    Insert,
    /// Schema syntax; individual native operations still require consuming review.
    Schema,
    Unsupported,
}

impl QueryOperation {
    fn classify(statement: &Statement) -> Self {
        match statement {
            Statement::Read(_) | Statement::Crosstab(_) => Self::Read,
            Statement::Update(_) => Self::Update,
            Statement::Delete(_) => Self::Delete,
            Statement::Insert(_) => Self::Insert,
            Statement::Definition(_) => Self::Schema,
            Statement::MakeTable(_) => Self::Schema,
        }
    }
}

/// Inert SQL captured by runtime inspection. Fields have no public constructors
/// or mutation access. This grants no permission to mutate a database or bypass
/// execution admission. Debug/Serde are omitted to avoid implicit SQL logging.
pub struct InspectedQuery {
    sql: String,
    operation: QueryOperation,
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

    /// The actual parsed statement family, independent of later admission.
    pub const fn operation(&self) -> QueryOperation {
        self.operation
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
            operation: QueryOperation::classify(&parsed.query().statement),
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
        options: QueryRunOptions,
        clock: &dyn Clock,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        self.require_query_inspection(target, cancelled)?;
        let mut output = Presentation::with_budget(options.presentation().budget(), cancelled);
        parameters::validate_input_limits(inputs, &mut output)
            .map_err(super::options::presentation_error)?;
        let parsed = parse_query(target.sql(), ParseLimits::default())?;
        let bindings = parameters::bindings(parsed.query(), inputs, &mut output)
            .map_err(super::options::presentation_error)?;
        check(cancelled)?;
        let issuer = self.query_result_issuer();
        let source = NativeSource::new_with_snapshot_limits(
            &mut self.source,
            SCOPE,
            options.native().budget(),
            cancelled,
            options.snapshot(),
        )
        .map_err(super::options::native_error)?
        .with_collections();
        let comparison_pool = RefCell::new(options.comparison().budget());
        let (mut source, resolver) =
            source.with_deferred_database_collation_resolver(&comparison_pool)?;
        execute(
            parsed.query(),
            &mut source,
            &resolver,
            &bindings,
            RunSettings {
                options,
                title: "Query results",
                clock,
                cancelled,
                issuer,
            },
            output,
        )
    }
}
