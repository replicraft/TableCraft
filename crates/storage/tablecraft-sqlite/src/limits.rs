use tablecraft_schema::{NameComparison, SchemaPolicy};

pub(crate) const MAX_TABLES: usize = 256;
pub(crate) const MAX_VALUE_BYTES: usize = 1_048_576;
pub(crate) const MAX_SQL_BYTES: usize = 262_144;
pub(crate) const MAX_EXPRESSION_DEPTH: i32 = 100;
pub(crate) const MAX_VM_PROGRAM_OPS: i32 = 100_000;
pub(crate) const VM_CHECK_INTERVAL: i32 = 100;
pub(crate) const SCHEMA_POLICY: SchemaPolicy = SchemaPolicy {
    max_name_bytes: 255,
    max_columns: 256,
    names: NameComparison::AsciiInsensitive,
};
