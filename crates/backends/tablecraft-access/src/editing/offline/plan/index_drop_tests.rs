//! Exact DROP family refuses before native construction without its source feature.
use super::{Cause, MutationIntent};
use tablecraft_ace::IndexDropIntentError;
use tablecraft_core::limits::ResourceBudget;
#[test]
fn exact_index_drop_prefix_never_falls_through_without_creation_source_authority() {
    for bytes in [
        b"TCID".as_slice(),
        b"TCID0001".as_slice(),
        b"TCID0001invalid".as_slice(),
        b"TCID0002".as_slice(),
    ] {
        assert!(matches!(
            MutationIntent::decode(bytes, &mut ResourceBudget::new(0, 0, 0), &|| false),
            Err(Cause::IndexDropIntent(
                IndexDropIntentError::UnsupportedSource
            ))
        ));
    }
}
