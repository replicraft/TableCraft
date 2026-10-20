use super::{calendar, character, retrieval};
use clap::Subcommand;
use tablecraft_expressions::functions::dates::TwoDigitYearWindow;
use tablecraft_expressions::functions::text::code_page::CharacterCodePage;

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Reconstruct one native saved local SELECT; prints SQL without executing it.
    #[command(
        after_help = "Reads explicitly selected native metadata from a stable/offline copy. Supports the qualified local SELECT profile; unknown, conflicting and unsupported fragments fail. This does not bind functions, expand saved sources or execute SQL. Use inspect --query for raw fragments of other query kinds."
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
        after_help = "Accepts one SELECT of scalar expressions with optional aliases. Database sources, query clauses, parameters and action statements are not supported. Text comparison requires a collation and is not available here."
    )]
    Eval {
        /// Last year of DateSerial's 100-year window; default 2029 (1930..2029).
        #[arg(long = "two-digit-year-max", value_name = "YEAR", value_parser = calendar::parse_window)]
        year_window: Option<TwoDigitYearWindow>,
        /// Explicit numeric character-code page; omitted leaves high byte codes unsupported.
        #[arg(long, value_name = "ID", value_parser = character::parse_code_page)]
        character_code_page: Option<CharacterCodePage>,
        /// SELECT text, passed as one shell-quoted argument.
        #[arg(allow_hyphen_values = true)]
        sql: String,
    },
    /// Execute bounded retrieval over inline rows, MDB/ACCDB or SQLite tables.
    #[command(
        after_help = "Supports inline SELECT/UNION sources, filtering, joins, grouping and subqueries. Use --database with a stable/offline copy for supported local scalar tables; use --saved NAME instead of SQL to execute a qualified saved SELECT. Add --multivalues for qualified stored .Value projections/grouping or --collections for those lists plus named attachment metadata; scalar-parent criteria use bounded membership matching. Joined/mixed parent criteria and selected indexes on that parent remain unsupported. Local saved SELECT sources resolve under shared limits and cycle checks. Linked sources and unsupported column types fail explicitly. Parameters use NAME=TYPE:VALUE (boolean, byte, short, long, bigint, text, date, guid or null). Text comparisons require an explicit --collation; ordinal is not an Access locale collation. LIKE also requires --like-mode ansi89 or ansi92; database wildcard settings are not inferred."
    )]
    Run {
        /// Last year of DateSerial's 100-year window; default 2029 (1930..2029).
        #[arg(long = "two-digit-year-max", value_name = "YEAR", value_parser = calendar::parse_window)]
        year_window: Option<TwoDigitYearWindow>,
        /// Explicit numeric character-code page; omitted leaves high byte codes unsupported.
        #[arg(long, value_name = "ID", value_parser = character::parse_code_page)]
        character_code_page: Option<CharacterCodePage>,
        /// Retrieval SQL text, passed as one shell-quoted argument.
        #[arg(
            allow_hyphen_values = true,
            required_unless_present = "saved",
            conflicts_with = "saved"
        )]
        sql: Option<String>,
        /// Execute this explicitly named native saved SELECT instead of SQL text.
        #[arg(
            long,
            value_name = "NAME",
            requires = "database",
            conflicts_with = "sql"
        )]
        saved: Option<String>,
        /// Declared parameter, for example Floor=bigint:4 or Label=text:hello.
        #[arg(long = "param", value_name = "NAME=TYPE:VALUE")]
        parameters: Vec<String>,
        /// Explicit text comparison policy.
        #[arg(long, value_enum)]
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
        /// Cumulative native read/allocation bytes; default 16 MiB, maximum 256 MiB.
        #[arg(long, value_name = "BYTES", requires = "database", value_parser = clap::value_parser!(u64).range(1..=crate::native::MAX_QUERY_READ_BYTES))]
        native_bytes: Option<u64>,
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
        collation: Option<retrieval::TextCollation>,
    },
}
