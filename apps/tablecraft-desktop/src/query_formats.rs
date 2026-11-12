//! Explicit startup conversion profiles; no OS or display preference inference.
use tablecraft_runtime::{QueryExpressionFormats, QueryExpressionFormatsError};

pub(super) fn number(value: &str) -> Result<QueryExpressionFormats, QueryExpressionFormatsError> {
    QueryExpressionFormats::from_profile_names(Some(value), None)
}

pub(super) fn date(value: &str) -> Result<QueryExpressionFormats, QueryExpressionFormatsError> {
    QueryExpressionFormats::from_profile_names(None, Some(value))
}
