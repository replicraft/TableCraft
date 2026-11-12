//! Exact native declarations and complete SELECT INTO rows from one captured image.
mod shape;

use crate::NativeSource;
use std::cell::RefCell;
use tablecraft_ace::{
    CreateDatabaseError, DatabaseQueryMode, FileFormat, FirstTableDefinition, InspectError,
    NativeImage, TableSnapshotLimits, inspect_table_schema,
};
#[cfg(feature = "journal")]
use tablecraft_ace::{PopulatedAddTableError, PopulatedAddTablePlan};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::predicates::LikeMode;
use tablecraft_model::ObjectKind;
use tablecraft_query::{ExecutionOptions, MakeTableError, plan_make_table};
use tablecraft_schema::NameComparison;
use tablecraft_sql::query::Query;
use thiserror::Error;

/// Failure before native make-table publication has been qualified.
#[derive(Debug, Error)]
pub enum NativeMakeTableError {
    /// The statement/source declaration is outside the qualified local projection slice.
    #[error("native make-table shape is not supported")]
    Shape,
    /// A table or saved query already uses the requested destination name.
    #[error("make-table destination already exists")]
    DestinationExists,
    /// Native metadata, caller budget, allocation or cancellation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The exact physical declaration cannot be represented by the creation owner.
    #[error(transparent)]
    Declaration(#[from] CreateDatabaseError),
    /// Atomic populated native construction refused the captured request.
    #[cfg(feature = "journal")]
    #[error(transparent)]
    Populated(#[from] PopulatedAddTableError),
    /// Complete query evaluation failed; no partial rows are returned.
    #[error(transparent)]
    Query(#[from] MakeTableError),
}

/// Data-only preparation from one immutable original, not a native candidate.
/// This type grants no permission to create, journal, overwrite or save a table.
#[must_use = "retain the bound data for a future atomic native operation or discard it"]
pub struct PreparedNativeMakeTable {
    original: NativeImage,
    definition: FirstTableDefinition,
    rows: Vec<Vec<Value>>,
}

impl PreparedNativeMakeTable {
    /// Consume the same original declaration and complete rows into one atomic
    /// native candidate. No SQL is reexecuted and no empty table is saved first.
    #[cfg(feature = "journal")]
    pub fn finish(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PopulatedAddTablePlan, NativeMakeTableError> {
        Ok(self
            .original
            .plan_populated_add_table(self.definition, self.rows, budget, cancelled)?)
    }

    /// Exact original from which both physical declarations and values were read.
    pub fn original(&self) -> &NativeImage {
        &self.original
    }

    /// Manual declarations without inherited generation, keys or field policies.
    pub fn definition(&self) -> &FirstTableDefinition {
        &self.definition
    }

    /// Complete selected rows in query order, with original NULL carriers intact.
    pub fn rows(&self) -> &[Vec<Value>] {
        &self.rows
    }

    /// Move preparation data without creating save or recovery authority.
    pub fn into_parts(self) -> (NativeImage, FirstTableDefinition, Vec<Vec<Value>>) {
        (self.original, self.definition, self.rows)
    }
}

/// Resolve native fields and qualified fixed computed outputs through a source constructed
/// internally over the same owned immutable image. No external Source or staged
/// result can be substituted. This does not qualify the source for publication.
///
/// The slice admits one local table, direct native fields and explicitly named
/// row-local fixed scalar expressions, WHERE, selection predicates and direct-field
/// ORDER BY. Computed aliases are a current native name qualification, not an
/// Access language restriction. Computed Text/Decimal declarations, source-expanding
/// expressions, groups, named/linked sources, external selectors and owner access
/// refuse. Bare and qualified source wildcards expand through actual native field
/// declarations. This entry refuses SQL parameter declarations; use the explicit
/// parameterized entry for that profile. Direct fields keep actual native sizes.
/// Fixed expression declarations come from complete typed query staging; no native
/// width, Required policy, primary key or default is inferred from selected rows.
/// Complete row retention obeys the caller's native/query/snapshot allowances;
/// this preparation imposes no additional row-count ceiling.
///
/// # Errors
/// Refuses unsupported syntax/declarations, existing destinations, incomplete
/// native reads, invalid names, query errors, cancellation and exhausted budgets.
pub fn prepare_native_make_table(
    image: NativeImage,
    query: Query,
    native: &RefCell<ResourceBudget>,
    query_budget: &mut ResourceBudget,
    parameters: &[(&str, Value)],
    options: ExecutionOptions<'_>,
    snapshot_limits: TableSnapshotLimits,
) -> Result<PreparedNativeMakeTable, NativeMakeTableError> {
    prepare_with_policy(
        image,
        query,
        native,
        query_budget,
        Preparation {
            parameters,
            options,
            snapshot_limits,
            native_comparison: None,
            absent_like_mode: None,
            declared_parameters: false,
        },
    )
}

/// Prepare using the captured image's actual database default and per-field
/// native comparison descriptors, with a separate cumulative comparison pool.
/// The native collation, resolver and LIKE mode override the corresponding
/// `options` fields. Unknown or absent database policies remain unavailable;
/// caller options cannot provide a fallback. Other host expression providers,
/// including the clock and random state, are retained. All shape and source
/// checks are shared with the explicit-policy preparation entry point.
pub fn prepare_native_make_table_with_native_comparison(
    image: NativeImage,
    query: Query,
    native: &RefCell<ResourceBudget>,
    comparison: &RefCell<ResourceBudget>,
    query_budget: &mut ResourceBudget,
    options: ExecutionOptions<'_>,
    snapshot_limits: TableSnapshotLimits,
) -> Result<PreparedNativeMakeTable, NativeMakeTableError> {
    prepare_with_policy(
        image,
        query,
        native,
        query_budget,
        Preparation {
            parameters: &[],
            options,
            snapshot_limits,
            native_comparison: Some(comparison),
            absent_like_mode: None,
            declared_parameters: false,
        },
    )
}

/// Typed inputs and read controls for explicitly parameterized SELECT INTO.
/// Parameters are supplied stored carriers, never defaults or SQL expressions.
pub struct NativeMakeTableOptions<'a> {
    /// Complete bindings for the original declared parameter namespace.
    pub parameters: &'a [(&'a str, Value)],
    /// Query evaluation providers and cumulative cancellation policy.
    pub execution: ExecutionOptions<'a>,
    /// Caller-selected application default, used only for absent native mode.
    /// Known native modes override it; Unknown never uses this fallback.
    /// None retains the adapter's explicit unsupplied-mode contract.
    pub absent_like_mode: Option<LikeMode>,
    /// Native consumer limits, independent from query execution allowances.
    pub snapshot_limits: TableSnapshotLimits,
}

/// Prepare a declared-parameter SELECT INTO using the same captured image for
/// native fields, comparison policies and complete row evaluation. Parameter
/// values are consumed only during preparation; finish never reruns the query.
/// An explicit absent_like_mode supplies a host application default only when
/// native mode is absent; Unknown still refuses mode-dependent evaluation.
/// Direct projections retain their actual native declarations. Computed output
/// qualification remains unchanged and does not infer sizes from supplied values.
///
/// # Errors
/// Refuses invalid or incomplete bindings, unsupported source/output shapes,
/// existing destinations, native/query failures, cancellation and exhausted pools.
pub fn prepare_parameterized_native_make_table_with_native_comparison(
    image: NativeImage,
    query: Query,
    native: &RefCell<ResourceBudget>,
    comparison: &RefCell<ResourceBudget>,
    query_budget: &mut ResourceBudget,
    options: NativeMakeTableOptions<'_>,
) -> Result<PreparedNativeMakeTable, NativeMakeTableError> {
    prepare_with_policy(
        image,
        query,
        native,
        query_budget,
        Preparation {
            parameters: options.parameters,
            options: options.execution,
            snapshot_limits: options.snapshot_limits,
            native_comparison: Some(comparison),
            absent_like_mode: options.absent_like_mode,
            declared_parameters: true,
        },
    )
}

struct Preparation<'a> {
    parameters: &'a [(&'a str, Value)],
    options: ExecutionOptions<'a>,
    snapshot_limits: TableSnapshotLimits,
    native_comparison: Option<&'a RefCell<ResourceBudget>>,
    absent_like_mode: Option<LikeMode>,
    declared_parameters: bool,
}

fn prepare_with_policy(
    image: NativeImage,
    query: Query,
    native: &RefCell<ResourceBudget>,
    query_budget: &mut ResourceBudget,
    preparation: Preparation<'_>,
) -> Result<PreparedNativeMakeTable, NativeMakeTableError> {
    let Preparation {
        parameters,
        options,
        snapshot_limits,
        native_comparison: comparison,
        absent_like_mode,
        declared_parameters,
    } = preparation;
    let _span = tracing::debug_span!("access.prepare_native_make_table").entered();
    let never_cancelled = || false;
    let cancelled = options.cancelled.unwrap_or(&never_cancelled);
    {
        let mut work = native
            .try_borrow_mut()
            .map_err(|_| InspectError::Unsupported)?;
        shape::check(&mut work, cancelled)?;
    }
    let header = image.catalog().header();
    if header.format() != FileFormat::Ace2016 || header.subversion() != 0 || header.encoded_pages()
    {
        return Err(NativeMakeTableError::Shape);
    }
    let select = shape::select(&query, declared_parameters)?;
    let destination = select.into.as_ref().ok_or(NativeMakeTableError::Shape)?;
    let scope = image.catalog().catalog().scope();
    let (schema, direct_definition) = {
        let mut work = native
            .try_borrow_mut()
            .map_err(|_| InspectError::Unsupported)?;
        shape::check(&mut work, cancelled)?;
        for object in image.catalog().catalog().objects() {
            shape::charge_name(
                object.name().as_str(),
                destination.as_str(),
                &mut work,
                cancelled,
            )?;
            if matches!(object.kind(), ObjectKind::Table | ObjectKind::Query)
                && NameComparison::AsciiInsensitive
                    .equals(object.name().as_str(), destination.as_str())
            {
                return Err(NativeMakeTableError::DestinationExists);
            }
        }
        let table = shape::table(select)?;
        let schema = inspect_table_schema(
            &mut image.reader(),
            scope,
            table.name.as_str(),
            &mut work,
            cancelled,
        )?;
        let direct_definition = if shape::preflight(select, &schema, &mut work, cancelled)? {
            let columns = shape::columns(select, &schema, &mut work, cancelled)?;
            Some(FirstTableDefinition::capture_manual_native(
                destination.as_str(),
                &columns,
                &mut work,
                cancelled,
            )?)
        } else {
            None
        };
        (schema, direct_definition)
    };
    let staged = {
        let mut source = NativeSource::new_with_shared_budget_and_snapshot_limits(
            image.reader(),
            scope,
            native,
            cancelled,
            snapshot_limits,
        )?;
        match comparison {
            Some(comparison) => {
                let like_mode = match source.database_query_mode()? {
                    DatabaseQueryMode::Ansi89 => Some(LikeMode::Ansi89),
                    DatabaseQueryMode::Ansi92 => Some(LikeMode::Ansi92),
                    DatabaseQueryMode::Absent => absent_like_mode,
                    DatabaseQueryMode::Unknown => None,
                };
                let (source, resolver) =
                    source.with_deferred_database_collation_resolver(comparison)?;
                plan_make_table(
                    query,
                    &source,
                    query_budget,
                    parameters,
                    ExecutionOptions {
                        like_mode,
                        collation: Some(resolver.default_collation()),
                        collation_resolver: Some(&resolver),
                        ..options
                    },
                )?
            }
            None => plan_make_table(query, &source, query_budget, parameters, options)?,
        }
    };
    let definition = match direct_definition {
        Some(definition) => definition,
        None => {
            let mut work = native
                .try_borrow_mut()
                .map_err(|_| InspectError::Unsupported)?;
            let columns = shape::staged_columns(&staged, &schema, &mut work, cancelled)?;
            FirstTableDefinition::capture_manual_native(
                staged.destination(),
                &columns,
                &mut work,
                cancelled,
            )?
        }
    };
    let rows = staged.into_rows();
    Ok(PreparedNativeMakeTable {
        original: image,
        definition,
        rows,
    })
}

#[cfg(test)]
mod tests;
