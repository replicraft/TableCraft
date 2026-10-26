//! Explicit cursor-backed SELECT/WHERE publication; ordinary retrieval stays buffered.
mod publisher;

use super::{
    Error, execution_context, parameters, parse, retrieval::TextCollation, run_args::RunArgs,
};
use crate::{commands::ExecutionError, native, presentation::Format};
use publisher::Publisher;
use std::{cell::RefCell, io::Write};
use tablecraft_access::{
    ExternalNativeSource, LinkedNativeSource, NativeSource, RecomputedNativeSource,
};
use tablecraft_ace::{
    LongTextReadLimits, LongValueReadLimits, LongValueReadSource, TableSnapshotLimits,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::{
    PreparationOptions, Source, StreamError, prepare_with_options, source::CollationResolver,
};

pub(super) const MAX_BATCH_ROWS: u32 = 1_000;
const DEFAULT_BATCH_ROWS: u32 = 128;

impl RunArgs {
    pub(in crate::commands) fn stream_query(
        self,
        format: Format,
        password: Option<&str>,
        writer: &mut dyn Write,
    ) -> Result<(), ExecutionError> {
        let _span = tracing::debug_span!("cli.query.stream").entered();
        if matches!(format, Format::Table) {
            return Err(ExecutionError::Operation(
                "streaming queries require TSV; omit --format table".into(),
            ));
        }
        let path = self.database.as_deref().ok_or(Error::Parameter)?;
        let parsed = self.sql.as_deref().map(parse).transpose()?;
        if parsed.is_some() == self.saved.is_some() {
            return Err(Error::Parameter.into());
        }
        let controls = self.controls();
        let parameters = parameters::parse(&self.parameters)?;
        let bindings = parameters
            .iter()
            .map(|(name, value)| (name.as_str(), value.clone()))
            .collect::<Vec<_>>();
        let batch_rows = self.batch_rows.unwrap_or(DEFAULT_BATCH_ROWS);
        let mut budget = ResourceBudget::new(
            self.native_bytes.unwrap_or(native::DEFAULT_NATIVE_BYTES),
            self.native_rows.unwrap_or(native::DEFAULT_NATIVE_ROWS),
            self.native_steps.unwrap_or(native::DEFAULT_NATIVE_STEPS),
        );

        let file = native::open(path, password, &mut budget).map_err(Error::Input)?;
        let value_limits = self
            .native_value_bytes
            .map(LongValueReadLimits::new)
            .transpose()
            .map_err(Error::Native)?
            .unwrap_or_default();
        let text_limits = self
            .native_text_bytes
            .map(LongTextReadLimits::new)
            .transpose()
            .map_err(Error::Native)?
            .unwrap_or_default();
        let file = LongValueReadSource::new(file, value_limits).with_text_limits(text_limits);
        let snapshot_limits = TableSnapshotLimits::new(
            u32::try_from(self.native_rows.unwrap_or(native::DEFAULT_NATIVE_ROWS))
                .map_err(|_| Error::Parameter)?,
            batch_rows,
        )
        .map_err(Error::Native)?;
        let native_pool = RefCell::new(budget);
        let source = NativeSource::new_with_shared_budget_and_snapshot_limits(
            file,
            native::CATALOG_SCOPE,
            &native_pool,
            &|| false,
            snapshot_limits,
        )
        .map_err(Error::Native)?;
        let source = if self.stored_calculations {
            source.with_stored_calculations().map_err(Error::Native)?
        } else {
            source
        };
        let comparison_pool = RefCell::new(controls.comparison_limits.budget());
        let database_policy = matches!(controls.collation, Some(TextCollation::Database));
        let (mut source, database_collation) = if database_policy {
            let (source, policy) = source
                .with_database_collation_resolver(&comparison_pool)
                .map_err(Error::Native)?;
            (
                source,
                Some(policy.with_option_compare(
                    tablecraft_expressions::comparison::StringCompareMode::Database,
                )),
            )
        } else {
            (source, None)
        };
        let override_collation = controls
            .collation
            .filter(|_| !database_policy)
            .map(|policy| policy.build_with_budget(controls.comparison_limits.budget()))
            .transpose()
            .map_err(Error::Native)?;
        let collation = database_collation
            .as_ref()
            .map(|policy| policy.default_collation() as &dyn Collation)
            .or_else(|| {
                override_collation
                    .as_ref()
                    .map(|policy| policy as &dyn Collation)
            });
        let resolver = database_collation
            .as_ref()
            .map(|policy| policy as &dyn CollationResolver);

        let saved = self
            .saved
            .as_deref()
            .map(|name| source.read_saved_query(name, Default::default()))
            .transpose()
            .map_err(Error::SavedRead)?;
        let query = match (&saved, &parsed) {
            (Some(saved), None) => saved.query(),
            (None, Some(parsed)) => parsed.query(),
            _ => return Err(Error::Parameter.into()),
        };

        let input = self.native_input(password).ok_or(Error::Parameter)?;
        let targets = super::links::load(
            &source,
            &input,
            &native_pool,
            &comparison_pool,
            database_policy,
            snapshot_limits,
        )?;
        let mappings = targets.mappings(source.handle(), &native_pool)?;
        let linked = if self.links.is_empty() {
            None
        } else {
            Some(LinkedNativeSource::new(source.handle(), &mappings).map_err(Error::Links)?)
        };
        let external_targets = super::external::load(
            &input,
            &native_pool,
            &comparison_pool,
            database_policy,
            snapshot_limits,
        )?;
        let external_mappings = external_targets.mappings(&native_pool)?;
        let external = if self.external.is_empty() {
            None
        } else {
            Some(
                match &linked {
                    Some(linked) => ExternalNativeSource::with_links(linked, &external_mappings),
                    None => ExternalNativeSource::new(source.handle(), &external_mappings),
                }
                .map_err(Error::External)?,
            )
        };
        let selections = super::recomputed::group(&self.recomputed_columns)?;
        let requests = super::recomputed::requests(&selections);
        let recomputed = if requests.is_empty() {
            None
        } else {
            Some(
                RecomputedNativeSource::new(&source, &requests, Default::default())
                    .map_err(Error::Recomputed)?,
            )
        };
        let root_source: &dyn Source = linked.as_ref().map_or(&source as &dyn Source, |s| s);
        let root_source: &dyn Source = recomputed.as_ref().map_or(root_source, |s| s);
        let query_source: &dyn Source = external.as_ref().map_or(root_source, |s| s);

        let prepared = prepare_with_options(
            query,
            query_source,
            PreparationOptions {
                binding_depth: controls.binding_depth,
                declared_arithmetic: controls.declared_integral,
                reference_year: controls.reference_year,
            },
        )
        .map_err(Error::Retrieval)?;
        let names = prepared.output_names().ok_or(Error::Parameter)?;
        let mut publisher = Publisher::new(names, batch_rows, controls.output_bytes, writer)?;
        execution_context::with_context(&controls, collation, resolver, |options| {
            prepared
                .execute_progressive(query_source, &bindings, options, &mut |row| {
                    publisher.append(row)
                })
                .map_err(|error| match error {
                    StreamError::Query(error) => ExecutionError::from(Error::Retrieval(error)),
                    StreamError::Sink(error) => error,
                })?;
            publisher.finish()
        })
    }
}
