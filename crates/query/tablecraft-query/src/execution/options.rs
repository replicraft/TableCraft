use tablecraft_expressions::{
    comparison::Collation,
    evaluation::EvalLimits,
    functions::{
        error_messages::ErrorMessages, math::random::RndState, session::AccessSessionContext,
    },
};

/// Consumable ceilings; caller limits are clamped to the documented maxima.
#[derive(Clone, Copy, Debug)]
pub struct QueryLimits {
    /// Cumulative scalar/row payload and buffer allocation bytes (default 16 MiB).
    pub bytes: u64,
    /// Cumulative scanned/intermediate/output rows (default 100,000).
    pub rows: u64,
    /// Expression, comparison and row work (default 1,000,000).
    pub steps: u64,
}
impl QueryLimits {
    /// Maximum cumulative allocation bytes in the retrieval profile.
    pub const MAX_BYTES: u64 = 16 * 1024 * 1024;
    /// Maximum cumulative scanned/intermediate/output rows.
    pub const MAX_ROWS: u64 = 100_000;
    /// Default expression, comparison and row work units.
    pub const DEFAULT_STEPS: u64 = 1_000_000;
    /// Maximum explicitly requested expression, comparison and row work units.
    pub const MAX_STEPS: u64 = 100_000_000;
}
impl Default for QueryLimits {
    fn default() -> Self {
        Self {
            bytes: Self::MAX_BYTES,
            rows: Self::MAX_ROWS,
            steps: Self::DEFAULT_STEPS,
        }
    }
}
/// Explicit execution providers and cancellation supplied by the host. Comparison
/// callbacks must be deterministic and bounded; clocks and random sequences retain
/// their documented stateful behavior. No default text order or random state.
#[derive(Clone, Copy, Default)]
pub struct ExecutionOptions<'a> {
    /// Immutable localized messages selected by the caller, without locale discovery.
    pub error_messages: Option<&'a dyn ErrorMessages>,
    /// Explicit immutable Access expression-session metadata, not authorization.
    pub access_session: Option<AccessSessionContext<'a>>,
    /// Caller-owned sequential Rnd state, shared by nested calls; never stored in plans.
    pub random: Option<&'a RndState>,
    /// Explicit Unicode casing language, independent of database collation.
    pub case_locale: Option<&'a tablecraft_expressions::functions::text::casing::CaseLocale>,
    /// Host-selected clock, read only when clock expressions execute.
    pub clock: Option<&'a dyn tablecraft_expressions::functions::dates::clock::Clock>,
    /// Caller-issued first day for system week requests and WeekdayName defaults.
    pub first_day_of_week: Option<tablecraft_expressions::functions::dates::FirstDayOfWeek>,
    /// Caller-issued first-week rule for explicit system week options.
    pub first_week_of_year: Option<tablecraft_expressions::functions::dates::FirstWeekOfYear>,
    /// Actual/reference Gregorian year for omitted-year date text; no clock is read.
    pub reference_year: Option<tablecraft_expressions::functions::dates::parsing::ReferenceYear>,
    /// Explicit CStr date/time output profile, independent of number-format and locale discovery.
    pub date_format: Option<tablecraft_expressions::functions::text::cstr::DateTextFormat>,
    /// Explicit numeric Text input/output policy for conversion, recognition and formatting.
    pub number_format: Option<tablecraft_expressions::functions::conversions::text::NumberFormat>,
    /// Gregorian DateSerial year window; omitted uses the explicit 1930..2029 profile.
    /// This execution setting is shared by all nested/named queries, never source metadata.
    pub year_window: Option<tablecraft_expressions::functions::dates::TwoDigitYearWindow>,
    /// Explicit numeric-byte character decoding for this execution and its nested queries.
    pub character_code_page:
        Option<tablecraft_expressions::functions::text::code_page::CharacterCodePage>,
    /// Opt into the engineering declaration-sensitive profile (historical field name).
    /// Known Scalar/Variant provenance is required for qualified integral,
    /// Single-selected and legacy Date-envelope arithmetic. Two known Variants use the reviewed String/numeric
    /// comparison exception before reviewed effective numeric predicate coercion.
    /// Other comparison families retain their owning profiles.
    pub declared_integral: bool,
    /// Individual materialized Text bytes; omitted uses 64 KiB, clamped to 1 MiB.
    /// Shared allocation/work budgets and parser/binding limits remain separate.
    pub max_text_bytes: Option<usize>,
    /// Consumable execution ceilings.
    pub limits: QueryLimits,
    /// Explicit LIKE wildcard convention; required only when LIKE is evaluated.
    pub like_mode: Option<tablecraft_expressions::predicates::LikeMode>,
    /// Global comparison override without a resolver; otherwise the default
    /// for expressions without a field policy. String built-ins use this policy.
    pub collation: Option<&'a dyn Collation>,
    /// Opt into per-operation field-policy resolution. Default provenance and
    /// string built-ins use collation; explicit field identities use this host
    /// callback. Distinct contributing identities fail instead of guessing.
    /// None retains the existing global-override behavior, ignoring provenance.
    pub collation_resolver: Option<&'a dyn crate::source::CollationResolver>,
    /// Called before execution work; true cancels without returning partial rows.
    pub cancelled: Option<&'a dyn Fn() -> bool>,
}

impl ExecutionOptions<'_> {
    pub(crate) fn text_limit(self) -> usize {
        self.max_text_bytes
            .unwrap_or(EvalLimits::DEFAULT_TEXT_BYTES)
            .min(EvalLimits::MAX_TEXT_BYTES)
    }
}
