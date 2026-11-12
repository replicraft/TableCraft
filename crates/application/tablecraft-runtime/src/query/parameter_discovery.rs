//! Explicit metadata acquisition owns a prepared plan, never a live native source.
use super::{context::Context, inspection::InspectedQuery, parameters, present};
use crate::query::context::Environment;
use crate::{
    Error, ParameterInput, QueryRunOptions, Response, controls::SCOPE, display::Presentation,
    document::Document,
};
use std::{cell::RefCell, sync::Arc};
use tablecraft_access::NativeSource;
use tablecraft_query::{
    ExecutionOptions, ParameterDiscoveryMode, PreparationOptions, PreparedParameterQuery,
    parameter_discovery_mode, prepare_with_parameter_discovery,
};
use tablecraft_sql::{ParseLimits, parse_query};

/// Issued immutable query metadata scoped to the original document and SQL witness.
/// It retains no NativeSource, resource borrow, file lease or expression provider.
pub struct DiscoveredQuery {
    original: Arc<InspectedQuery>,
    mode: Mode,
}

enum Mode {
    Strict,
    Implicit(Box<PreparedParameterQuery>),
}

impl DiscoveredQuery {
    /// Original syntax inspection; the prepared owner cannot replace its witness.
    pub fn original(&self) -> &Arc<InspectedQuery> {
        &self.original
    }
    /// Whether actual metadata selected the opt-in unresolved-name profile.
    pub fn has_implicit_mode(&self) -> bool {
        matches!(self.mode, Mode::Implicit(_))
    }
}

impl Document {
    pub(crate) fn discover_query_parameters(
        &mut self,
        target: Arc<InspectedQuery>,
        options: QueryRunOptions,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        self.require_query_inspection(&target, cancelled)?;
        if target.operation() != super::inspection::QueryOperation::Read {
            return Err(tablecraft_query::QueryError::Unsupported.into());
        }
        let parsed = parse_query(target.sql(), ParseLimits::default())?;
        let mut output = Presentation::with_budget(options.presentation().budget(), cancelled);
        let mut query_budget = options.query().budget();
        let source = NativeSource::new_with_snapshot_limits(
            &mut self.source,
            SCOPE,
            options.native().budget(),
            cancelled,
            options.snapshot(),
        )
        .map_err(super::options::native_error)?
        .with_collections();
        let mode = parameter_discovery_mode(
            parsed.query(),
            &source,
            Default::default(),
            &mut query_budget,
            cancelled,
        )
        .map_err(super::options::query_error)?;
        let (mode, parameters) = match mode {
            ParameterDiscoveryMode::Strict => (
                Mode::Strict,
                parameters::specs(parsed.query(), &mut output)
                    .map_err(super::options::presentation_error)?,
            ),
            ParameterDiscoveryMode::Implicit => {
                let prepared = prepare_with_parameter_discovery(
                    parsed.query(),
                    &source,
                    PreparationOptions::default(),
                    &mut query_budget,
                    cancelled,
                )
                .map_err(super::options::query_error)?;
                let specs = parameters::discovered_specs(&prepared, parsed.query(), &mut output)
                    .map_err(super::options::presentation_error)?;
                output
                    .allocation(1, size_of::<PreparedParameterQuery>())
                    .map_err(super::options::presentation_error)?;
                (Mode::Implicit(Box::new(prepared)), specs)
            }
        };
        output
            .allocation(1, size_of::<DiscoveredQuery>() + 2 * size_of::<usize>())
            .map_err(super::options::presentation_error)?;
        Ok(Response::QueryParametersDiscovered {
            target: Arc::new(DiscoveredQuery {
                original: target,
                mode,
            }),
            parameters,
        })
    }

    pub(crate) fn run_discovered_query(
        &mut self,
        target: &DiscoveredQuery,
        inputs: &[ParameterInput],
        options: QueryRunOptions,
        environment: Environment<'_>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        self.require_query_inspection(&target.original, cancelled)?;
        let Mode::Implicit(prepared) = &target.mode else {
            // Metadata positively chose Strict before discovery; this is not a
            // retry after any opt-in bind or execution error.
            return self.run_query(&target.original, inputs, options, environment, cancelled);
        };
        let mut output = Presentation::with_budget(options.presentation().budget(), cancelled);
        parameters::validate_input_limits(inputs, &mut output)
            .map_err(super::options::presentation_error)?;
        let bindings = parameters::discovered_bindings(prepared, inputs, &mut output)
            .map_err(super::options::presentation_error)?;
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
        let context = Context::new(environment);
        let like_mode = super::like_mode::sql_view(
            source
                .database_query_mode()
                .map_err(super::options::native_error)?,
        );
        let result = prepared
            .execute_collections(
                &source,
                &bindings,
                ExecutionOptions {
                    cancelled: Some(cancelled),
                    limits: options.query().query_limits(),
                    collation: Some(resolver.default_collation()),
                    collation_resolver: Some(&resolver),
                    like_mode,
                    ..context.options()
                },
            )
            .map_err(super::options::query_error)?;
        present(result, "Query results", output, issuer).map_err(super::options::presentation_error)
    }
}
