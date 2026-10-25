use super::{Error, comparison_limits::Args as ComparisonArgs, output::TableOutput, parameters};
use crate::native;
use clap::ValueEnum;
use std::sync::{Arc, atomic::AtomicBool};
use std::{cell::RefCell, ffi::OsString, path::Path};
use tablecraft_access::{
    ExternalNativeSource, LinkedNativeSource, NativeSource, template::TemplateSource,
};
use tablecraft_ace::{
    LongTextReadLimits, LongValueReadLimits, LongValueReadSource, TableSnapshotLimits,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::comparison::Collation;
use tablecraft_expressions::functions::conversions::text::NumberFormat;
use tablecraft_expressions::functions::text::cstr::DateTextFormat;
use tablecraft_expressions::functions::{
    dates::TwoDigitYearWindow, text::code_page::CharacterCodePage,
};
use tablecraft_query::{
    ExpressionQueryResult, QueryLimits, Source, execute_with_schema_discovery,
    source::{CollationResolver, MemorySource},
};
use tablecraft_sql::query::Query;

pub(crate) use super::collation::TextCollation;
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Wildcards {
    /// Access asterisk, question mark, digit and negated-list wildcards.
    Ansi89,
    /// SQL-compatible percent, underscore and negated-list wildcards.
    Ansi92,
}
impl From<Wildcards> for tablecraft_expressions::predicates::LikeMode {
    fn from(mode: Wildcards) -> Self {
        match mode {
            Wildcards::Ansi89 => Self::Ansi89,
            Wildcards::Ansi92 => Self::Ansi92,
        }
    }
}
pub(super) enum Input<'a> {
    Sql(&'a Query),
    Saved(&'a str),
}
impl<'a> Input<'a> {
    fn sql(&self) -> Result<&'a Query, Error> {
        match self {
            Self::Sql(query) => Ok(query),
            Self::Saved(_) => Err(Error::Parameter),
        }
    }
}

pub(super) struct NativeInput<'a> {
    pub password: Option<&'a str>,
    pub path: &'a Path,
    pub links: &'a [OsString],
    pub external: &'a [OsString],
    pub external_index: &'a [String],
    pub bytes: u64,
    pub rows: u64,
    pub steps: u64,
    pub index: &'a [String],
    pub auto_index: bool,
    pub multivalues: bool,
    pub collections: bool,
    pub decoded_attachments: bool,
    pub stored_calculations: bool,
    pub attachment_bytes: Option<u32>,
    pub native_value_bytes: Option<u32>,
    pub native_text_bytes: Option<u32>,
}

pub(super) struct Controls {
    pub declared_integral: bool,
    pub max_text_bytes: Option<usize>,
    pub output_bytes: usize,
    pub execution_limits: QueryLimits,
    pub comparison_limits: ComparisonArgs,
    pub random_seed: u32,
    pub case_locale: Option<tablecraft_expressions::functions::text::casing::CaseLocale>,
    pub clock: Option<tablecraft_expressions::functions::dates::clock::ClockReading>,
    pub collation: Option<TextCollation>,
    pub text_policy: super::text_policy::Args,
    pub like_mode: Option<Wildcards>,
    pub year_window: Option<TwoDigitYearWindow>,
    pub character_code_page: Option<CharacterCodePage>,
    pub number_format: Option<NumberFormat>,
    pub date_format: Option<DateTextFormat>,
    pub first_week_of_year: Option<tablecraft_expressions::functions::dates::FirstWeekOfYear>,
    pub first_day_of_week: Option<tablecraft_expressions::functions::dates::FirstDayOfWeek>,
    pub reference_year: Option<tablecraft_expressions::functions::dates::parsing::ReferenceYear>,
}

pub(super) fn run(
    input: Input<'_>,
    raw: &[String],
    controls: Controls,
    native_input: Option<NativeInput<'_>>,
    sqlite: Option<&Path>,
) -> Result<String, Error> {
    let database_policy = matches!(controls.collation, Some(TextCollation::Database));
    if database_policy && native_input.is_none() {
        return Err(Error::Parameter);
    }
    let collation = if database_policy {
        None
    } else {
        controls
            .collation
            .map(|policy| policy.build_with_budget(controls.comparison_limits.budget()))
            .transpose()
            .map_err(Error::Native)?
    };
    let policy = collation.as_ref().map(|c| c as &dyn Collation);
    let parameters = parameters::parse(raw)?;
    let bindings = parameters
        .iter()
        .map(|(name, value)| (name.as_str(), value.clone()))
        .collect::<Vec<_>>();
    let result = if let Some(path) = sqlite {
        let source = tablecraft_sqlite::SqliteSource::open(
            path,
            native::budget(),
            Arc::new(AtomicBool::new(false)),
        )
        .map_err(Error::Sqlite)?;
        let result = execute(input.sql()?, &source, &bindings, &controls, policy, None)?;
        source.close().map_err(Error::Sqlite)?;
        result
    } else if let Some(input_config) = native_input {
        let snapshot_limits = TableSnapshotLimits::new(
            u32::try_from(input_config.rows).map_err(|_| Error::Parameter)?,
            TableSnapshotLimits::default().batch_rows(),
        )
        .map_err(Error::Native)?;
        let mut budget =
            ResourceBudget::new(input_config.bytes, input_config.rows, input_config.steps);
        if let Some(package) =
            native::open_template(input_config.path, &mut budget).map_err(Error::Template)?
        {
            if !input_config.links.is_empty()
                || !input_config.external.is_empty()
                || database_policy
                || !input_config.index.is_empty()
                || input_config.auto_index
                || input_config.multivalues
                || input_config.collections
                || input_config.decoded_attachments
                || input_config.stored_calculations
                || input_config.native_value_bytes.is_some()
                || input_config.native_text_bytes.is_some()
                || matches!(input, Input::Saved(_))
            {
                return Err(Error::Retrieval(tablecraft_query::QueryError::Unsupported));
            }
            let source =
                TemplateSource::new(&package, budget, &|| false).map_err(Error::Template)?;
            execute(input.sql()?, &source, &bindings, &controls, policy, None)?
        } else {
            let file = native::open(input_config.path, input_config.password, &mut budget)
                .map_err(Error::Input)?;
            let value_limits = input_config
                .native_value_bytes
                .map(LongValueReadLimits::new)
                .transpose()
                .map_err(Error::Native)?
                .unwrap_or_default();
            let text_limits = input_config
                .native_text_bytes
                .map(LongTextReadLimits::new)
                .transpose()
                .map_err(Error::Native)?
                .unwrap_or_default();
            let file = LongValueReadSource::new(file, value_limits).with_text_limits(text_limits);
            let native_pool = RefCell::new(budget);
            let source = NativeSource::new_with_shared_budget_and_snapshot_limits(
                file,
                native::CATALOG_SCOPE,
                &native_pool,
                &|| false,
                snapshot_limits,
            )
            .map_err(Error::Native)?;
            let source = super::native_options::configure(source, &input_config, value_limits)?;
            let comparison_pool = RefCell::new(controls.comparison_limits.budget());
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
            let mut source = match input_config.index {
                [] => source,
                [table, index] => source.with_index(table, index).map_err(Error::Native)?,
                _ => return Err(Error::Parameter),
            };
            let saved = match input {
                Input::Saved(name) => Some(
                    source
                        .read_saved_query(name, Default::default())
                        .map_err(Error::SavedRead)?,
                ),
                Input::Sql(_) => None,
            };
            let query = match &saved {
                Some(saved) => saved.query(),
                None => input.sql()?,
            };
            let targets = super::links::load(
                &source,
                &input_config,
                &native_pool,
                &comparison_pool,
                database_policy,
                snapshot_limits,
            )?;
            let mappings = targets.mappings(source.handle(), &native_pool)?;
            let linked = if input_config.links.is_empty() {
                None
            } else {
                Some(LinkedNativeSource::new(source.handle(), &mappings).map_err(Error::Links)?)
            };
            let external_targets = super::external::load(
                &input_config,
                &native_pool,
                &comparison_pool,
                database_policy,
                snapshot_limits,
            )?;
            let external_mappings = external_targets.mappings(&native_pool)?;
            let external = if input_config.external.is_empty() {
                None
            } else {
                Some(
                    match &linked {
                        Some(linked) => {
                            ExternalNativeSource::with_links(linked, &external_mappings)
                        }
                        None => ExternalNativeSource::new(source.handle(), &external_mappings),
                    }
                    .map_err(Error::External)?,
                )
            };
            let root_source: &dyn Source = linked.as_ref().map_or(&source as &dyn Source, |s| s);
            let query_source: &dyn Source = external.as_ref().map_or(root_source, |s| s);
            execute(
                query,
                query_source,
                &bindings,
                &controls,
                database_collation
                    .as_ref()
                    .map(|c| c.default_collation() as &dyn Collation)
                    .or(policy),
                database_collation
                    .as_ref()
                    .map(|c| c as &dyn CollationResolver),
            )?
        }
    } else {
        execute(
            input.sql()?,
            &MemorySource::default(),
            &bindings,
            &controls,
            policy,
            None,
        )?
    };
    let mut output = TableOutput::with_limit(controls.output_bytes);
    for (i, name) in result.columns().iter().enumerate() {
        if i > 0 {
            output.separator().map_err(Error::Output)?;
        }
        output.header(Some(name), i + 1).map_err(Error::Output)?;
    }
    output.newline().map_err(Error::Output)?;
    for row in result.rows() {
        for (i, value) in row.iter().enumerate() {
            if i > 0 {
                output.separator().map_err(Error::Output)?;
            }
            output.expression_value(value).map_err(Error::Output)?;
        }
        output.newline().map_err(Error::Output)?;
    }
    Ok(output.finish())
}

fn execute(
    query: &Query,
    source: &dyn Source,
    bindings: &[(&str, Value)],
    controls: &Controls,
    collation: Option<&dyn Collation>,
    collation_resolver: Option<&dyn CollationResolver>,
) -> Result<ExpressionQueryResult, Error> {
    super::execution_context::with_context(controls, collation, collation_resolver, |options| {
        execute_with_schema_discovery(query, source, bindings, options).map_err(Error::Retrieval)
    })
}
