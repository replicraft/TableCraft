//! Current retrieval profile limits; not universal schema or native format rules.
use tablecraft_schema::{NameComparison, SchemaPolicy};

pub(crate) const SOURCE_SCHEMA_POLICY: SchemaPolicy = SchemaPolicy {
    max_name_bytes: 255,
    max_columns: 256,
    names: NameComparison::AsciiInsensitive,
};
