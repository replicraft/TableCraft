//! Query Nz selects non-NULL input values or its explicit/empty-Text fallback.
use crate::OutputType;
use tablecraft_schema::ScalarType;

pub(super) fn result(input: OutputType, replacement: Option<OutputType>) -> OutputType {
    match input {
        OutputType::Null => {
            replacement.unwrap_or_else(|| OutputType::scalar(ScalarType::Text, false))
        }
        OutputType::Scalar {
            data_type,
            nullable: false,
        } => OutputType::Scalar {
            data_type,
            nullable: false,
        },
        OutputType::Scalar { data_type, .. } => {
            let non_null = OutputType::Scalar {
                data_type,
                nullable: false,
            };
            match replacement {
                Some(replacement) => non_null.union(replacement),
                // The empty replacement fits the existing Text length ceiling.
                None if data_type.scalar_type() == ScalarType::Text => non_null,
                // Other carriers can produce their value or empty Text.
                None => OutputType::Dynamic,
            }
        }
        OutputType::Dynamic => OutputType::Dynamic,
    }
}
