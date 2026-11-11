//! Exact native declarations and complete SELECT INTO rows from one captured image.
mod shape;

use crate::NativeSource;
use std::cell::RefCell;
use tablecraft_ace::{
    CreateDatabaseError, FileFormat, FirstTableDefinition, InspectError, NativeImage,
    TableSnapshotLimits, inspect_table_schema,
};
#[cfg(feature = "journal")]
use tablecraft_ace::{PopulatedAddTableError, PopulatedAddTablePlan};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::ObjectKind;
use tablecraft_query::{ExecutionOptions, MakeTableError, plan_make_table};
use tablecraft_schema::NameComparison;
use tablecraft_sql::query::Query;
use thiserror::Error;

/// Failure before native make-table publication has been qualified.
#[derive(Debug, Error)]
pub enum NativeMakeTableError {
    /// The statement/source declaration is outside the direct local projection slice.
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

/// Resolve direct native field shapes and evaluate through a source constructed
/// internally over the same owned immutable image. No external Source or staged
/// result can be substituted. This does not qualify the source for publication.
///
/// The initial slice admits a single local table and explicit field projections,
/// aliases and direct-field ORDER BY. Filters, groups, computed projections,
/// wildcards, named/linked sources, external selectors and owner access refuse.
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
        },
    )
}

/// Prepare using the captured image's actual database default and per-field
/// native comparison descriptors, with a separate cumulative comparison pool.
/// This entry point selects native policies; caller-supplied comparers are not
/// accepted or silently replaced. All other shape and source checks are shared
/// with the explicit-policy preparation entry point.
pub fn prepare_native_make_table_with_native_comparison(
    image: NativeImage,
    query: Query,
    native: &RefCell<ResourceBudget>,
    comparison: &RefCell<ResourceBudget>,
    query_budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    snapshot_limits: TableSnapshotLimits,
) -> Result<PreparedNativeMakeTable, NativeMakeTableError> {
    prepare_with_policy(
        image,
        query,
        native,
        query_budget,
        Preparation {
            parameters: &[],
            options: ExecutionOptions {
                cancelled: Some(cancelled),
                ..Default::default()
            },
            snapshot_limits,
            native_comparison: Some(comparison),
        },
    )
}

struct Preparation<'a> {
    parameters: &'a [(&'a str, Value)],
    options: ExecutionOptions<'a>,
    snapshot_limits: TableSnapshotLimits,
    native_comparison: Option<&'a RefCell<ResourceBudget>>,
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
    let select = shape::select(&query)?;
    let destination = select.into.as_ref().ok_or(NativeMakeTableError::Shape)?;
    let scope = image.catalog().catalog().scope();
    let definition = {
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
        let columns = shape::columns(select, &schema, &mut work, cancelled)?;
        FirstTableDefinition::capture_manual_native(
            destination.as_str(),
            &columns,
            &mut work,
            cancelled,
        )?
    };
    let staged = {
        let source = NativeSource::new_with_shared_budget_and_snapshot_limits(
            image.reader(),
            scope,
            native,
            cancelled,
            snapshot_limits,
        )?;
        match comparison {
            Some(comparison) => {
                let (source, resolver) =
                    source.with_deferred_database_collation_resolver(comparison)?;
                plan_make_table(
                    query,
                    &source,
                    query_budget,
                    parameters,
                    ExecutionOptions {
                        collation: Some(resolver.default_collation()),
                        collation_resolver: Some(&resolver),
                        ..options
                    },
                )?
            }
            None => plan_make_table(query, &source, query_budget, parameters, options)?,
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
