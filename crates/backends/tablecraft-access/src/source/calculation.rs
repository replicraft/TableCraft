//! Explicit bounded row-local previews, separate from native cached results.
mod dependencies;
mod syntax;
use super::NativeSource;
use tablecraft_ace::{
    CalculationDeclaration, InspectError, NativeReadSource, inspect_calculation_declaration,
};
use tablecraft_core::limits::Resource;
use tablecraft_query::{ExecutionOptions, PreparedQuery, QueryError, QueryResult, Source, prepare};
use tablecraft_sql::{ParseError, ParseLimits, query::Query};
use thiserror::Error;

/// Redacted failures while explicitly preparing or evaluating a stored formula.
#[derive(Debug, Error)]
pub enum CalculationError {
    /// Native metadata or read controls refused the operation.
    #[error("calculation metadata is unavailable")]
    Native(#[from] InspectError),
    /// Stored expression syntax is invalid or exceeds the explicit parser limits.
    #[error("calculation expression could not be parsed")]
    Parse(#[from] ParseError),
    /// Formula is outside the explicit pure same-row preview profile.
    #[error("calculation expression is outside the supported row-local profile")]
    Unsupported,
    /// Binding, source, evaluation or query resource controls refused the preview.
    #[error("calculation query failed")]
    Query(#[from] QueryError),
}

/// A metadata-only prepared preview of one native calculated expression.
/// No cached value was inspected, and no freshness/native execution is implied.
pub struct PreparedCalculation<'a> {
    source: &'a dyn Source,
    declaration: CalculationDeclaration,
    dependencies: Vec<String>,
    calculated_dependencies: Vec<String>,
    query: Query,
    plan: PreparedQuery,
}

pub(super) struct CalculationDefinition {
    pub(super) declaration: CalculationDeclaration,
    pub(super) dependencies: Vec<String>,
    pub(super) calculated_dependencies: Vec<String>,
    pub(super) query: Query,
}

impl PreparedCalculation<'_> {
    /// Original qualified expression/result metadata, without cached payloads.
    pub fn declaration(&self) -> &CalculationDeclaration {
        &self.declaration
    }

    /// Distinct ordinary same-table fields in first-reference order.
    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    /// Distinct recomputed same-table calculated dependencies, in first-reference order.
    /// The root declaration is separate; cached values are never used as inputs.
    pub fn calculated_dependencies(&self) -> &[String] {
        &self.calculated_dependencies
    }

    /// Validated row-local query model. The output is explicitly named Recomputed.
    pub fn query(&self) -> &Query {
        &self.query
    }

    /// Explicitly recompute using TableCraft's current evaluator and source.
    /// Ordinary source schema checks, cumulative query controls, field policies
    /// and eager failures apply. Result carriers follow expression promotion;
    /// they are not coerced to the declared stored carrier, saved or substituted
    /// for cached values. The preview borrows its original source; another source
    /// cannot be substituted. Its unchanged/offline ownership contract still applies.
    pub fn execute(&self, options: ExecutionOptions<'_>) -> Result<QueryResult, CalculationError> {
        let _span = tracing::debug_span!("native.calculation.preview").entered();
        Ok(self.plan.execute(self.source, &[], options)?)
    }
}

impl<R: NativeReadSource> NativeSource<'_, R> {
    /// Prepare one explicitly selected stored formula without reading user rows.
    /// Pure same-row arithmetic, concatenation, comparison, logical operators and
    /// NULL tests may use ordinary fields or qualified calculated dependencies.
    /// Dependencies are recomputed under bounded cycle-checked expansion, without
    /// cached inputs or intermediate declared-type conversion. Calls other than
    /// deterministic exact-arity IIf, subqueries, complex/nonlocal inputs and
    /// unresolved parameters remain refused. This neither
    /// changes ordinary query eligibility nor inspects/replaces cached results.
    pub fn prepare_calculation(
        &self,
        table: &str,
        column: &str,
        limits: ParseLimits,
    ) -> Result<PreparedCalculation<'_>, CalculationError> {
        let _span = tracing::debug_span!("native.calculation.prepare").entered();
        let definition = self.calculation_definition(table, column, limits)?;
        let plan = prepare(&definition.query, self)?;
        Ok(PreparedCalculation {
            source: self,
            declaration: definition.declaration,
            dependencies: definition.dependencies,
            calculated_dependencies: definition.calculated_dependencies,
            query: definition.query,
            plan,
        })
    }

    pub(super) fn calculation_definition(
        &self,
        table: &str,
        column: &str,
        limits: ParseLimits,
    ) -> Result<CalculationDefinition, CalculationError> {
        let entry = self.entry(table)?;
        let declaration = {
            let mut source = self
                .source
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            inspect_calculation_declaration(
                &mut *source,
                self.scope,
                &entry.name,
                column,
                &mut budget,
                self.cancelled,
            )?
        };
        let columns = self.schema_columns(table)?;
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| QueryError::Source)?;
        let (expression, calculated_dependencies) =
            dependencies::expand(&declaration, limits, &mut budget, self.cancelled)?;
        let (query, dependencies) = syntax::prepare_expression(
            &entry.name,
            expression,
            columns,
            &mut budget,
            self.cancelled,
        )?;
        // The separate cached-query policy must not turn cached calculations
        // into ordinary dependencies of the restricted recomputation preview.
        for dependency in &dependencies {
            if (self.cancelled)() {
                return Err(QueryError::Cancelled.into());
            }
            budget
                .charge(Resource::Steps, columns.len() as u64 + 1)
                .map_err(QueryError::from)?;
            let index = columns
                .iter()
                .position(|column| column.name() == dependency)
                .ok_or(CalculationError::Unsupported)?;
            if self
                .entry(table)?
                .columns
                .get()
                .ok_or(QueryError::Source)?
                .cached
                .get(index)
                .is_some_and(Option::is_some)
            {
                return Err(CalculationError::Unsupported);
            }
        }
        Ok(CalculationDefinition {
            declaration,
            dependencies,
            calculated_dependencies,
            query,
        })
    }
}
