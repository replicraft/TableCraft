mod dispatch;

use super::comparison_limits::Args as ComparisonArgs;
use super::execution_limits::Args as ExecutionArgs;
use super::large_values::Args as LargeValueArgs;
use super::retrieval::{TextCollation, Wildcards};
use super::{calendar, character, clock, date_text, number};
use clap::Args as ClapArgs;
use std::{ffi::OsString, path::PathBuf};
use tablecraft_ace::{AttachmentDecodeLimits, LongTextReadLimits, LongValueReadLimits};
use tablecraft_expressions::functions::conversions::text::NumberFormat;
use tablecraft_expressions::functions::dates::clock::ClockReading;
use tablecraft_expressions::functions::dates::parsing::ReferenceYear;
use tablecraft_expressions::functions::dates::{
    FirstDayOfWeek, FirstWeekOfYear, TwoDigitYearWindow,
};
use tablecraft_expressions::functions::math::random::RndState;
use tablecraft_expressions::functions::text::casing::CaseLocale;
use tablecraft_expressions::functions::text::code_page::CharacterCodePage;
use tablecraft_expressions::functions::text::cstr::DateTextFormat;

#[derive(ClapArgs)]
#[command(group(clap::ArgGroup::new("comparison_context").args(["collation", "text_locale"]).multiple(true)))]
pub(crate) struct RunArgs {
    /// Stream eligible local SELECT/WHERE, COUNT(*) and UNION ALL results as TSV;
    /// later errors may leave earlier batches.
    #[arg(long, requires = "database", conflicts_with_all = ["saved", "sqlite", "index", "auto_index", "links", "external", "external_index", "multivalues", "collections", "decoded_attachments", "stored_calculations"])]
    pub(crate) stream: bool,
    /// Rows per streaming batch; default 128, maximum 1,000.
    #[arg(long, requires = "stream", value_name = "ROWS", value_parser = clap::value_parser!(u32).range(1..=super::stream::MAX_BATCH_ROWS as i64))]
    pub(crate) batch_rows: Option<u32>,
    /// Last year of DateSerial's 100-year window; default 2029 (1930..2029).
    #[arg(long = "two-digit-year-max", value_name = "YEAR", value_parser = calendar::parse_window)]
    pub(crate) year_window: Option<TwoDigitYearWindow>,
    /// Explicit numeric character-code page; omitted leaves high byte codes unsupported.
    #[arg(long, value_name = "ID", value_parser = character::parse_code_page)]
    pub(crate) character_code_page: Option<CharacterCodePage>,
    /// Numeric Text input and numeric formatting profile; no ambient locale.
    #[arg(long, value_name = "PROFILE", value_parser = number::parse_format)]
    pub(crate) number_format: Option<NumberFormat>,
    /// Explicit Gregorian date parsing/output syntax, independent of numeric formatting.
    #[arg(long, value_name = "PROFILE", value_parser = date_text::parse_format)]
    pub(crate) date_format: Option<DateTextFormat>,
    /// Host first day for system week settings: 1=Sunday through 7=Saturday.
    #[arg(long, value_name = "DAY", value_parser = calendar::parse_first_day)]
    pub(crate) first_day_of_week: Option<FirstDayOfWeek>,
    /// Host rule for system first-week requests: 1=Jan-1, 2=four-day, 3=full week.
    #[arg(long, value_name = "RULE", value_parser = calendar::parse_first_week)]
    pub(crate) first_week_of_year: Option<FirstWeekOfYear>,
    /// Explicit Unicode casing language for LCase/UCase (for example en-US, tr, or und).
    #[arg(long, value_name = "LANGUAGE")]
    pub(crate) case_locale: Option<CaseLocale>,
    /// Fixed local clock for Date()/Time()/Now()/Timer(); otherwise use the local system clock.
    #[arg(long, value_name = "LOCAL-DATETIME", value_parser = clock::parse_reading)]
    pub(crate) clock: Option<ClockReading>,
    /// Raw 24-bit Rnd state for this command; omission uses the deterministic Basic seed.
    #[arg(long, value_name = "SEED", default_value_t = RndState::DEFAULT_SEED,
        value_parser = clap::value_parser!(u32).range(0..=i64::from(RndState::MAX_SEED)))]
    pub(crate) random_seed: u32,
    /// Explicit year for omitted-year SQL literals and date text; the clock is not read.
    #[arg(long, value_name = "YEAR", value_parser = date_text::parse_reference_year)]
    pub(crate) reference_year: Option<ReferenceYear>,
    /// Retrieval SQL text, passed as one shell-quoted argument.
    #[arg(
        allow_hyphen_values = true,
        required_unless_present = "saved",
        conflicts_with = "saved"
    )]
    pub(crate) sql: Option<String>,
    /// Execute this explicitly named native saved SELECT/UNION/crosstab instead of SQL text.
    #[arg(
        long,
        value_name = "NAME",
        requires = "database",
        conflicts_with = "sql"
    )]
    pub(crate) saved: Option<String>,
    /// Declared parameter: Floor=bigint:4, Price=currency:0.0001 or Label=text:hello.
    #[arg(long = "param", value_name = "NAME=TYPE:VALUE")]
    pub(crate) parameters: Vec<String>,
    /// Explicit ordinal/General override, or stored database/per-field General policies.
    #[arg(long, value_enum, requires_if("database", "database"))]
    pub(crate) collation: Option<TextCollation>,
    #[command(flatten)]
    pub(crate) text_policy: super::text_policy::Args,
    /// Explicit LIKE wildcards; database ANSI settings are not inferred.
    #[arg(long, value_enum)]
    pub(crate) like_mode: Option<Wildcards>,
    /// Read supported local tables from this explicit stable/offline MDB/ACCDB copy.
    #[arg(long, value_name = "PATH")]
    pub(crate) database: Option<PathBuf>,
    /// Explicitly authorize a linked alias to read an already chosen stable database; repeat for more aliases.
    #[arg(long = "link", value_names = ["ALIAS", "PATH"], num_args = super::links::MAPPING_FIELDS,
        action = clap::ArgAction::Append, requires = "database", conflicts_with = "sqlite")]
    pub(crate) links: Vec<OsString>,
    /// Bind an exact SQL IN or per-table database token to this explicitly chosen stable database; repeat for more tokens.
    #[arg(long = "external", value_names = ["DATABASE_TOKEN", "PATH"], num_args = super::external::MAPPING_FIELDS,
        action = clap::ArgAction::Append, requires = "database", conflicts_with = "sqlite")]
    pub(crate) external: Vec<OsString>,
    /// Require this index on one explicitly bound external target's local table.
    #[arg(long, value_names = ["DATABASE_TOKEN", "TABLE", "INDEX"], num_args = super::external::INDEX_FIELDS,
        action = clap::ArgAction::Set, requires = "external", conflicts_with = "auto_index")]
    pub(crate) external_index: Vec<String>,
    /// Query qualified stored calculation caches; results may be stale and are never recalculated.
    #[arg(long, requires = "database", conflicts_with = "sqlite")]
    pub(crate) stored_calculations: bool,
    /// Enable qualified .Value expansion and scalar-parent membership criteria.
    #[arg(long, requires = "database")]
    pub(crate) multivalues: bool,
    /// Enable stored .Value lists and attachment/history members, including encoded FileData.
    #[arg(long, requires = "database", conflicts_with = "multivalues")]
    pub(crate) collections: bool,
    /// Decode selected attachment FileData into original file bytes.
    #[arg(long, requires = "collections", conflicts_with = "multivalues")]
    pub(crate) decoded_attachments: bool,
    /// Maximum decoded attachment file bytes; default 1 MiB, maximum 16 MiB. Encoded storage limits still apply.
    #[arg(long, value_name = "BYTES", requires_all = ["database", "decoded_attachments"], value_parser = clap::value_parser!(u32).range(1..=i64::from(AttachmentDecodeLimits::MAX_FILE_BYTES)))]
    pub(crate) attachment_bytes: Option<u32>,
    /// Maximum encoded Memo/OLE/attachment value bytes; default 2 MiB, maximum 32 MiB.
    #[arg(long, value_name = "BYTES", requires = "database", conflicts_with = "sqlite", value_parser = clap::value_parser!(u32).range(1..=i64::from(LongValueReadLimits::MAX_BYTES)))]
    pub(crate) native_value_bytes: Option<u32>,
    /// Maximum decoded Memo UTF-8 bytes; default 1 MiB, maximum 16 MiB. Query Text limits remain separate.
    #[arg(long, value_name = "BYTES", requires = "database", conflicts_with = "sqlite", value_parser = clap::value_parser!(u32).range(1..=i64::from(LongTextReadLimits::MAX_BYTES)))]
    pub(crate) native_text_bytes: Option<u32>,
    /// Cumulative native read/allocation bytes; default 16 MiB, maximum 256 MiB.
    #[arg(long, value_name = "BYTES", requires = "database", conflicts_with = "sqlite", value_parser = clap::value_parser!(u64).range(1..=crate::native::MAX_QUERY_READ_BYTES))]
    pub(crate) native_bytes: Option<u64>,
    /// Cumulative native rows and scalar/collection-table snapshot ceiling; default 16,384.
    #[arg(long, value_name = "ROWS", requires = "database", conflicts_with = "sqlite", value_parser = clap::value_parser!(u64).range(1..=crate::native::MAX_NATIVE_READ_ROWS))]
    pub(crate) native_rows: Option<u64>,
    /// Cumulative native work allowance; default 1,000,000, maximum 100,000,000.
    #[arg(long, value_name = "STEPS", requires = "database", conflicts_with = "sqlite", value_parser = clap::value_parser!(u64).range(1..=crate::native::MAX_NATIVE_READ_STEPS))]
    pub(crate) native_steps: Option<u64>,
    #[command(flatten)]
    pub(crate) execution_limits: Box<ExecutionArgs>,
    #[command(flatten)]
    pub(crate) comparison_limits: ComparisonArgs,
    /// Engineering literal typing and declaration-sensitive arithmetic/comparisons.
    /// Native qualification remains pending.
    #[arg(long = "declared-arithmetic", visible_alias = "declared-integral")]
    pub(crate) declared_integral: bool,
    #[command(flatten)]
    pub(crate) large_values: Box<LargeValueArgs>,
    /// Read supported scalar tables from an existing SQLite file, read-only.
    #[arg(long, value_name = "PATH", conflicts_with_all = ["database", "index"])]
    pub(crate) sqlite: Option<PathBuf>,
    /// Require a supported native index for this table's snapshot.
    #[arg(long, value_names = ["TABLE", "INDEX"], num_args = 2, requires = "database", action = clap::ArgAction::Set)]
    pub(crate) index: Vec<String>,
    /// Choose a qualified native index for safe scalar filters; unfiltered reads stay sequential.
    #[arg(long, requires = "database", conflicts_with_all = ["index", "sqlite"])]
    pub(crate) auto_index: bool,
}
