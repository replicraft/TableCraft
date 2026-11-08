use crate::source::{CollationResolver, TextCollationId};
use tablecraft_core::values::Value;
use tablecraft_expressions::comparison::Collation;

/// Explicit typed inputs, comparison providers and field provenance for one frozen mutation.
/// Policy identities remain opaque; this layer never chooses native comparers.
#[derive(Default)]
pub struct MutationOptions<'a> {
    /// Exact typed inputs for all declarations. None preserves legacy refusal.
    /// Values are literal carriers, never SQL fragments or expressions.
    pub parameters: Option<&'a [(&'a str, Value)]>,
    /// Explicit comparison for expressions without a stored field policy.
    pub collation: Option<&'a dyn Collation>,
    /// Resolve field identities in the source's stable policy namespace.
    pub collation_resolver: Option<&'a dyn CollationResolver>,
    /// Policies aligned with snapshot columns; empty supplies no column identities.
    pub text_collations: &'a [Option<TextCollationId>],
}
