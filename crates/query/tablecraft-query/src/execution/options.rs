use tablecraft_expressions::comparison::Collation;

/// Consumable ceilings; larger caller limits are clamped to these defaults.
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
    /// Maximum expression, comparison and row work units.
    pub const MAX_STEPS: u64 = 1_000_000;
}
impl Default for QueryLimits {
    fn default() -> Self {
        Self {
            bytes: Self::MAX_BYTES,
            rows: Self::MAX_ROWS,
            steps: Self::MAX_STEPS,
        }
    }
}
/// Explicit collation and cancellation supplied by the host. Callbacks must be
/// deterministic and enforce their own I/O/work limits; no default text order.
#[derive(Default)]
pub struct ExecutionOptions<'a> {
    /// Gregorian DateSerial year window; omitted uses the explicit 1930..2029 profile.
    /// This execution setting is shared by all nested/named queries, never source metadata.
    pub year_window: Option<tablecraft_expressions::functions::dates::TwoDigitYearWindow>,
    /// Explicit numeric-byte character decoding for this execution and its nested queries.
    pub character_code_page:
        Option<tablecraft_expressions::functions::text::code_page::CharacterCodePage>,
    /// Consumable execution ceilings.
    pub limits: QueryLimits,
    /// Explicit LIKE wildcard convention; required only when LIKE is evaluated.
    pub like_mode: Option<tablecraft_expressions::predicates::LikeMode>,
    /// Text comparison policy, if provided.
    pub collation: Option<&'a dyn Collation>,
    /// Called before execution work; true cancels without returning partial rows.
    pub cancelled: Option<&'a dyn Fn() -> bool>,
}
