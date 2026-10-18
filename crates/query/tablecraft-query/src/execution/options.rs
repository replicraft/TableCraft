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
impl Default for QueryLimits {
    fn default() -> Self {
        Self {
            bytes: 16 * 1024 * 1024,
            rows: 100_000,
            steps: 1_000_000,
        }
    }
}
/// Explicit collation and cancellation supplied by the host. Callbacks must be
/// deterministic and enforce their own I/O/work limits; no default text order.
#[derive(Default)]
pub struct ExecutionOptions<'a> {
    /// Consumable execution ceilings.
    pub limits: QueryLimits,
    /// Text comparison policy, if provided.
    pub collation: Option<&'a dyn Collation>,
    /// Called before execution work; true cancels without returning partial rows.
    pub cancelled: Option<&'a dyn Fn() -> bool>,
}
