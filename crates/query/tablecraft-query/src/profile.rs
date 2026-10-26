//! Current retrieval profile limits; not universal schema or native format rules.

use tablecraft_expressions::evaluation::EvalLimits;
use tablecraft_schema::{NameComparison, SchemaPolicy};

pub(crate) const SOURCE_SCHEMA_POLICY: SchemaPolicy = SchemaPolicy {
    max_name_bytes: 255,
    max_columns: 256,
    names: NameComparison::AsciiInsensitive,
};

// Binding shape/work bounds are independent of native storage/layout limits.
// Access's published nested-query ceiling; each definition keeps its own syntax limit.
pub(crate) const MAX_NAMED_DEPTH: usize = 50;
// Runtime domain calls are a distinct recursion boundary, unchanged by named nesting.
pub(crate) const MAX_DOMAIN_DEPTH: usize = 16;
// Engineering active-interpreter allowance, shared by runtime reads and
// suspended named-definition syntax; not a native Access or raw stack-frame limit.
pub(crate) const MAX_RUNTIME_READ_DEPTH: usize = 128;
pub(crate) const MAX_BIND_NODES: usize = 2048;
pub(crate) const MAX_BIND_BYTES: usize = 1_048_576;
pub(crate) const MAX_PARAMETERS: usize = 256;
pub(crate) const MAX_UNION_BRANCHES: usize = 256;
pub(crate) const MAX_IN_VALUES: usize = 256;
pub(crate) const MAX_FROM_ITEMS: usize = 32;
pub(crate) const MAX_GROUP_KEYS: usize = 10;
pub(crate) const MAX_SORT_KEYS: usize = 10;
pub(crate) const MAX_TEXT_BYTES: usize = EvalLimits::DEFAULT_TEXT_BYTES;
pub(crate) const PERCENT_TOTAL: u32 = 100;
