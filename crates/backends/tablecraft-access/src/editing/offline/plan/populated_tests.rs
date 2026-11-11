//! TCPT has no legacy fallback when native construction authority is absent.
use super::{Cause, MutationIntent};
use tablecraft_core::limits::ResourceBudget;

#[test]
fn populated_prefix_is_terminal_without_creation_authority() {
    for raw in [b"TCPT0001".as_slice(), b"TCPT0002".as_slice()] {
        assert!(matches!(
            MutationIntent::decode(raw, &mut ResourceBudget::new(0, 0, 0), &|| false,),
            Err(Cause::UnsupportedSource)
        ));
    }
}

#[test]
fn index_creation_prefix_is_terminal_without_creation_authority() {
    for raw in [b"TCIC0001".as_slice(), b"TCIC0002".as_slice()] {
        assert!(matches!(
            MutationIntent::decode(raw, &mut ResourceBudget::new(0, 0, 0), &|| false),
            Err(Cause::UnsupportedSource)
        ));
    }
}
