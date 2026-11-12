//! Exact fresh creation remains terminal before oldTCIC routing without native creation.
use super::{Cause, MutationIntent};
use tablecraft_ace::FreshIndexCreateIntentError;
use tablecraft_core::limits::ResourceBudget;
#[test]
fn fresh_creation_prefix_never_enters_old_decoder_without_creation_source_authority() {
    for raw in [b"TCIC0002".as_slice(), b"TCIC0002invalid".as_slice()] {
        assert!(matches!(
            MutationIntent::decode(raw, &mut ResourceBudget::new(0, 0, 0), &|| false),
            Err(Cause::FreshIndexCreationIntent(
                FreshIndexCreateIntentError::UnsupportedSource
            ))
        ));
    }
}
