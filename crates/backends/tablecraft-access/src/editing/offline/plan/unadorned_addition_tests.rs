//! The new exact family never falls through to an older ADD decoder.
use super::{Cause, MutationIntent};
use tablecraft_ace::UnadornedFieldAdditionIntentError;
use tablecraft_core::limits::ResourceBudget;

#[test]
fn unadorned_addition_prefix_is_terminal_without_creation_authority() {
    for raw in [b"TCAC0014".as_slice(), b"TCAC0014invalid".as_slice()] {
        assert!(matches!(
            MutationIntent::decode(raw, &mut ResourceBudget::new(0, 0, 0), &|| false),
            Err(Cause::UnadornedFieldAdditionIntent(
                UnadornedFieldAdditionIntentError::UnsupportedSource
            ))
        ));
    }
}
