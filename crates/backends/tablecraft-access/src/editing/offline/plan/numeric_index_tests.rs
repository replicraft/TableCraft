//! Exact numeric original is terminal without constructive native authority.
use super::{Cause, MutationIntent};
use tablecraft_ace::NumericIndexCreateIntentError;
use tablecraft_core::limits::ResourceBudget;

#[test]
fn exact_numeric_prefix_does_not_retry_an_old_index_original_without_creation() {
    for raw in [b"TCIC0003".as_slice(), b"TCIC0003invalid".as_slice()] {
        assert!(matches!(
            MutationIntent::decode(raw, &mut ResourceBudget::new(0, 0, 0), &|| false),
            Err(Cause::NumericIndexCreationIntent(
                NumericIndexCreateIntentError::UnsupportedSource
            ))
        ));
    }
}
