use super::{calendar, character, clock, date_text, number, retrieval};
use clap::Subcommand;
use tablecraft_expressions::functions::conversions::text::NumberFormat;
use tablecraft_expressions::functions::dates::TwoDigitYearWindow;
use tablecraft_expressions::functions::math::random::RndState;
use tablecraft_expressions::functions::text::code_page::CharacterCodePage;
use tablecraft_expressions::functions::text::cstr::DateTextFormat;

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Reconstruct one native saved SELECT/UNION/crosstab; prints SQL without executing it.
    #[command(
        after_help = "Reads explicitly selected native metadata from a stable/offline copy. Supports the qualified SELECT/UNION/crosstab profile; unknown, conflicting and unsupported fragments fail. This does not bind functions, expand saved sources or execute SQL. Use inspect --query for raw fragments of other query kinds."
    )]
    Saved {
        /// Native query name, passed as one shell-quoted argument.
        #[arg(value_name = "NAME")]
        name: String,
        /// Explicit stable/offline MDB/ACCDB copy, opened read-only.
        #[arg(long, value_name = "PATH")]
        database: std::path::PathBuf,
    },
    /// Parse one Access query and print normalized SQL; does not execute it.
    Parse {
        /// SQL text, passed as one shell-quoted argument.
        #[arg(allow_hyphen_values = true)]
        sql: String,
    },
    /// Evaluate a source-free SELECT using the supported scalar expression rules.
    #[command(
        after_help = "Accepts one SELECT of scalar expressions with optional aliases. Database sources, query clauses, parameters and action statements are not supported. Comparisons between two Text values require a collation and are not available here."
    )]
    Eval {
        /// Last year of DateSerial's 100-year window; default 2029 (1930..2029).
        #[arg(long = "two-digit-year-max", value_name = "YEAR", value_parser = calendar::parse_window)]
        year_window: Option<TwoDigitYearWindow>,
        /// Explicit numeric character-code page; omitted leaves high byte codes unsupported.
        #[arg(long, value_name = "ID", value_parser = character::parse_code_page)]
        character_code_page: Option<CharacterCodePage>,
        /// Numeric Text input and numeric formatting profile; no ambient locale.
        #[arg(long, value_name = "PROFILE", value_parser = number::parse_format)]
        number_format: Option<NumberFormat>,
        /// Explicit Gregorian date parsing/output syntax, independent of numeric formatting.
        #[arg(long,value_name="PROFILE",value_parser=date_text::parse_format)]
        date_format: Option<DateTextFormat>,
        /// Host first day for system week settings: 1=Sunday through 7=Saturday.
        #[arg(long,value_name="DAY",value_parser=calendar::parse_first_day)]
        first_day_of_week: Option<tablecraft_expressions::functions::dates::FirstDayOfWeek>,
        /// Host rule for system first-week requests: 1=Jan-1, 2=four-day, 3=full week.
        #[arg(long,value_name="RULE",value_parser=calendar::parse_first_week)]
        first_week_of_year: Option<tablecraft_expressions::functions::dates::FirstWeekOfYear>,
        /// Explicit Unicode casing language for LCase/UCase (for example en-US, tr, or und).
        #[arg(long, value_name = "LANGUAGE")]
        case_locale: Option<tablecraft_expressions::functions::text::casing::CaseLocale>,
        /// Fixed local clock for Date()/Time()/Now()/Timer(); otherwise use the local system clock.
        #[arg(long,value_name="LOCAL-DATETIME",value_parser=clock::parse_reading)]
        clock: Option<tablecraft_expressions::functions::dates::clock::ClockReading>,
        /// Raw 24-bit Rnd state for this command; omission uses the deterministic Basic seed.
        #[arg(long, value_name = "SEED", default_value_t = RndState::DEFAULT_SEED,
            value_parser = clap::value_parser!(u32).range(0..=i64::from(RndState::MAX_SEED)))]
        random_seed: u32,
        /// Explicit year for date text that omits its year; the clock is not read.
        #[arg(long,value_name="YEAR",value_parser=date_text::parse_reference_year)]
        reference_year: Option<tablecraft_expressions::functions::dates::parsing::ReferenceYear>,
        /// SELECT text, passed as one shell-quoted argument.
        #[arg(allow_hyphen_values = true)]
        sql: String,
    },
    /// Execute bounded retrieval over inline rows, MDB/ACCDB or SQLite tables.
    #[command(
        after_help = "Supports inline SELECT/UNION/crosstab sources, filtering, joins, grouping and subqueries. Use --database with a stable/offline copy for supported local scalar tables; use --saved NAME instead of SQL to execute a qualified saved SELECT/UNION/crosstab. Add --multivalues for qualified stored .Value projections/grouping or --collections for those lists plus named attachment metadata; scalar-parent criteria use bounded membership matching. Collection queries support joined/mixed WHERE membership, aligned multi-member atoms, correlated subqueries and crosstabs under an explicit profile; native Access SQL qualification remains pending. ON member fields expand child rows. Selected indexes on collection-reading parents remain unsupported. Larger sequential scalar snapshots require explicit native row/work allowances; --execution-steps controls the separate query work pool. Index/collection structural ceilings and query byte/row/output caps remain. Local saved SELECT/UNION sources resolve under shared limits and cycle checks. Qualified fixed/computed named crosstab schemas bind without scans; dynamic or policy-dependent schemas use explicit execution-stage discovery under the shared query budget. Linked sources and unsupported column types fail explicitly. Parameters use NAME=TYPE:VALUE (boolean, byte, short, long, bigint, text, date, guid or null); bracket the name as [NAME]=TYPE:VALUE when it contains an equals sign. Text comparisons require an explicit --collation: ordinal retains Unicode order; general-legacy and general-2010 apply a provisional full-string weight model under a separate cumulative 16 MiB/1,000,000-step budget. Use database with --database to select stored General default/per-field policies under the same shared allowance; unknown required field policies and unresolved conflicts fail. Opted-in .Value and attachment Text members retain their stored field policies; decoded FileData stays Binary. These provisional profiles are not native SQL qualification; family flags remain explicit global overrides. LIKE keeps Unicode-scalar units for ordinal; explicit General policies use UTF-16 wildcard units and complete weighted literal/list spans. General pattern spaces remain literal; negated classes use excluded-prefix precedence. Native qualification remains pending. General database/default substring search uses bounded full-weight spans, including expansions; text-mode/LCID search remains unsupported. LIKE also requires --like-mode ansi89 or ansi92; database wildcard settings are not inferred."
    )]
    Run {
        /// Last year of DateSerial's 100-year window; default 2029 (1930..2029).
        #[arg(long = "two-digit-year-max", value_name = "YEAR", value_parser = calendar::parse_window)]
        year_window: Option<TwoDigitYearWindow>,
        /// Explicit numeric character-code page; omitted leaves high byte codes unsupported.
        #[arg(long, value_name = "ID", value_parser = character::parse_code_page)]
        character_code_page: Option<CharacterCodePage>,
        /// Numeric Text input and numeric formatting profile; no ambient locale.
        #[arg(long, value_name = "PROFILE", value_parser = number::parse_format)]
        number_format: Option<NumberFormat>,
        /// Explicit Gregorian date parsing/output syntax, independent of numeric formatting.
        #[arg(long,value_name="PROFILE",value_parser=date_text::parse_format)]
        date_format: Option<DateTextFormat>,
        /// Host first day for system week settings: 1=Sunday through 7=Saturday.
        #[arg(long,value_name="DAY",value_parser=calendar::parse_first_day)]
        first_day_of_week: Option<tablecraft_expressions::functions::dates::FirstDayOfWeek>,
        /// Host rule for system first-week requests: 1=Jan-1, 2=four-day, 3=full week.
        #[arg(long,value_name="RULE",value_parser=calendar::parse_first_week)]
        first_week_of_year: Option<tablecraft_expressions::functions::dates::FirstWeekOfYear>,
        /// Explicit Unicode casing language for LCase/UCase (for example en-US, tr, or und).
        #[arg(long, value_name = "LANGUAGE")]
        case_locale: Option<tablecraft_expressions::functions::text::casing::CaseLocale>,
        /// Fixed local clock for Date()/Time()/Now()/Timer(); otherwise use the local system clock.
        #[arg(long,value_name="LOCAL-DATETIME",value_parser=clock::parse_reading)]
        clock: Option<tablecraft_expressions::functions::dates::clock::ClockReading>,
        /// Raw 24-bit Rnd state for this command; omission uses the deterministic Basic seed.
        #[arg(long, value_name = "SEED", default_value_t = RndState::DEFAULT_SEED,
            value_parser = clap::value_parser!(u32).range(0..=i64::from(RndState::MAX_SEED)))]
        random_seed: u32,
        /// Explicit year for date text that omits its year; the clock is not read.
        #[arg(long,value_name="YEAR",value_parser=date_text::parse_reference_year)]
        reference_year: Option<tablecraft_expressions::functions::dates::parsing::ReferenceYear>,
        /// Retrieval SQL text, passed as one shell-quoted argument.
        #[arg(
            allow_hyphen_values = true,
            required_unless_present = "saved",
            conflicts_with = "saved"
        )]
        sql: Option<String>,
        /// Execute this explicitly named native saved SELECT/UNION/crosstab instead of SQL text.
        #[arg(
            long,
            value_name = "NAME",
            requires = "database",
            conflicts_with = "sql"
        )]
        saved: Option<String>,
        /// Declared parameter: Floor=bigint:4, Price=currency:0.0001 or Label=text:hello.
        #[arg(long = "param", value_name = "NAME=TYPE:VALUE")]
        parameters: Vec<String>,
        /// Explicit ordinal/General override, or stored database/per-field General policies.
        #[arg(long, value_enum, requires_if("database", "database"))]
        collation: Option<retrieval::TextCollation>,
        /// Explicit LIKE wildcards; database ANSI settings are not inferred.
        #[arg(long, value_enum)]
        like_mode: Option<retrieval::Wildcards>,
        /// Read supported local tables from this explicit stable/offline MDB/ACCDB copy.
        #[arg(long, value_name = "PATH")]
        database: Option<std::path::PathBuf>,
        /// Enable qualified .Value expansion and scalar-parent membership criteria.
        #[arg(long, requires = "database")]
        multivalues: bool,
        /// Enable stored .Value lists and named attachment metadata members.
        #[arg(long, requires = "database", conflicts_with = "multivalues")]
        collections: bool,
        /// Expose FileData as bounded decoded Binary; native SQL representation is unqualified.
        #[arg(long, requires = "collections", conflicts_with = "multivalues")]
        decoded_attachments: bool,
        /// Cumulative native read/allocation bytes; default 16 MiB, maximum 256 MiB.
        #[arg(long, value_name = "BYTES", requires = "database", conflicts_with = "sqlite", value_parser = clap::value_parser!(u64).range(1..=crate::native::MAX_QUERY_READ_BYTES))]
        native_bytes: Option<u64>,
        /// Cumulative native row allowance and sequential scalar snapshot ceiling; default 16,384.
        #[arg(long, value_name = "ROWS", requires = "database", conflicts_with = "sqlite", value_parser = clap::value_parser!(u64).range(1..=crate::native::MAX_NATIVE_READ_ROWS))]
        native_rows: Option<u64>,
        /// Cumulative native work allowance; default 1,000,000, maximum 100,000,000.
        #[arg(long, value_name = "STEPS", requires = "database", conflicts_with = "sqlite", value_parser = clap::value_parser!(u64).range(1..=crate::native::MAX_NATIVE_READ_STEPS))]
        native_steps: Option<u64>,
        /// Query comparison/evaluation work; default 1,000,000, maximum 100,000,000.
        #[arg(long, value_name = "STEPS", value_parser = clap::value_parser!(u64).range(1..=tablecraft_query::QueryLimits::MAX_STEPS))]
        execution_steps: Option<u64>,
        /// Engineering declared integral overflow; native Access qualification remains pending.
        #[arg(long)]
        declared_integral: bool,
        #[command(flatten)]
        large_values: Box<super::large_values::Args>,
        /// Read supported scalar tables from an existing SQLite file, read-only.
        #[arg(long, value_name = "PATH", conflicts_with_all = ["database", "index"])]
        sqlite: Option<std::path::PathBuf>,
        /// Require a supported native index for this table's snapshot.
        #[arg(long, value_names = ["TABLE", "INDEX"], num_args = 2, requires = "database", action = clap::ArgAction::Set)]
        index: Vec<String>,
    },
    /// Apply a bounded single-table UPDATE to a fresh offline Jet 4 copy.
    #[command(
        after_help = "Supports one existing non-indexed non-NULL fixed Int32 assignment with pure expressions and WHERE. Requires an offline Jet 4 source and a new output path; never modifies the input. ACE, INSERT/DELETE, joins, parameters and unavailable rules/content are rejected. A write failure after output creation may leave an incomplete copy. Text filters require an explicit collation; ordinal is not an Access locale collation."
    )]
    Update {
        /// UPDATE text, passed as one shell-quoted argument.
        #[arg(allow_hyphen_values = true)]
        sql: String,
        /// Existing stable/offline MDB input, opened read-only under an OS lock.
        #[arg(long, value_name = "PATH")]
        database: std::path::PathBuf,
        /// Fresh output copy; an existing path is never overwritten.
        #[arg(long, value_name = "PATH")]
        output: std::path::PathBuf,
        /// Explicit text comparison policy.
        #[arg(long, value_enum)]
        collation: Option<super::collation::UpdateCollation>,
    },
}
