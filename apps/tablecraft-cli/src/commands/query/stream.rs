//! Explicit cursor-backed SELECT/WHERE publication; ordinary retrieval stays buffered.
mod publisher;

use super::{
    Error, execution_context, parameters, parse, retrieval::TextCollation, run_args::RunArgs,
};
use crate::{commands::ExecutionError, native, presentation::Format};
use publisher::Publisher;
use std::{cell::RefCell, io::Write};
use tablecraft_access::NativeSource;
use tablecraft_ace::{
    LongTextReadLimits, LongValueReadLimits, LongValueReadSource, TableSnapshotLimits,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::{
    PreparationOptions, StreamError, prepare_with_options, source::CollationResolver,
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
        let sql = self.sql.as_deref().ok_or(Error::Parameter)?;
        let query = parse(sql)?;
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
        let comparison_pool = RefCell::new(controls.comparison_limits.budget());
        let database_policy = matches!(controls.collation, Some(TextCollation::Database));
        let (source, database_collation) = if database_policy {
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

        let prepared = prepare_with_options(
            query.query(),
            &source,
            PreparationOptions {
                declared_arithmetic: controls.declared_integral,
                reference_year: controls.reference_year,
            },
        )
        .map_err(Error::Retrieval)?;
        let names = prepared.output_names().ok_or(Error::Parameter)?;
        let mut publisher = Publisher::new(names, batch_rows, controls.output_bytes, writer)?;
        execution_context::with_context(&controls, collation, resolver, |options| {
            prepared
                .execute_progressive(&source, &bindings, options, &mut |row| {
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
