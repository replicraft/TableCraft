use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::comparison::Collation;
use tablecraft_expressions::functions::error_messages::ErrorMessages;
use tablecraft_query::QueryLimits;

/// Owned per-request controls suitable for moving to the database worker.
pub struct AsyncOptions {
    /// Request-local immutable message provider; omission leaves Error unconfigured.
    pub error_messages: Option<Arc<dyn ErrorMessages>>,
    /// Owned Access expression-session name; this grants no authentication or
    /// permissions. None leaves CurrentUser without a session. Validated on use.
    pub access_user_name: Option<String>,
    /// Optional raw 24-bit Rnd seed. The worker creates one request-local state;
    /// omission leaves Rnd without a provider rather than choosing ambient state.
    pub random_seed: Option<u32>,
    /// Request-local explicit casing language.
    pub case_locale: Option<tablecraft_expressions::functions::text::casing::CaseLocale>,
    /// Owned request-local host clock; the worker never discovers one implicitly.
    pub clock: Option<Arc<dyn tablecraft_expressions::functions::dates::clock::Clock>>,
    /// Caller-issued first day for system week requests and WeekdayName defaults.
    pub first_day_of_week: Option<tablecraft_expressions::functions::dates::FirstDayOfWeek>,
    /// Caller-issued first-week rule for explicit system week options.
    pub first_week_of_year: Option<tablecraft_expressions::functions::dates::FirstWeekOfYear>,
    /// Caller-owned year for omitted-year date text, isolated to this request.
    pub reference_year: Option<tablecraft_expressions::functions::dates::parsing::ReferenceYear>,
    /// Explicit per-request CStr date/time output profile.
    pub date_format: Option<tablecraft_expressions::functions::text::cstr::DateTextFormat>,
    /// Explicit per-request numeric Text input/output profile, independent of SQLite affinity.
    pub number_format: Option<tablecraft_expressions::functions::conversions::text::NumberFormat>,
    /// Cumulative metadata, row, allocation and SQLite VM work allowance.
    pub storage_budget: ResourceBudget,
    /// Explicit engineering declaration-sensitive arithmetic/comparisons (historical name).
    pub declared_integral: bool,
    /// Individual query Text bytes; omitted keeps the standard default.
    pub max_text_bytes: Option<usize>,
    /// Separate TableCraft evaluation/materialization ceilings.
    pub query_limits: QueryLimits,
    /// Explicit wildcard convention for TableCraft LIKE evaluation.
    pub like_mode: Option<tablecraft_expressions::predicates::LikeMode>,
    /// Optional per-request Gregorian DateSerial window; omission retains 1930..2029.
    pub year_window: Option<tablecraft_expressions::functions::dates::TwoDigitYearWindow>,
    /// Optional request-local numeric character-code page, without ambient locale access.
    pub character_code_page:
        Option<tablecraft_expressions::functions::text::code_page::CharacterCodePage>,
    /// Optional explicit text ordering/equality policy, callable on the worker.
    pub collation: Option<Arc<dyn Collation + Send + Sync>>,
    /// Request-specific cancellation flag. Dropping the request future sets it.
    /// Use a fresh flag for each request; cancellation is not automatically reset.
    pub cancelled: Arc<AtomicBool>,
}

impl AsyncOptions {
    /// Supply explicit storage limits and cancellation; use the standard bounded
    /// query limits and no implicit text collation.
    pub fn new(storage_budget: ResourceBudget, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            error_messages: None,
            access_user_name: None,
            random_seed: None,
            storage_budget,
            query_limits: QueryLimits::default(),
            declared_integral: false,
            max_text_bytes: None,
            collation: None,
            like_mode: None,
            year_window: None,
            character_code_page: None,
            number_format: None,
            case_locale: None,
            clock: None,
            date_format: None,
            reference_year: None,
            first_day_of_week: None,
            first_week_of_year: None,
            cancelled,
        }
    }
}
