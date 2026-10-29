use super::*;
use sha2::{Digest, Sha256};

#[test]
fn separately_derived_french_dictionary_has_the_reviewed_fixed_hash() {
    let dictionary = dictionary().unwrap();
    let actual = Sha256::digest(dictionary.as_fst().as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        actual,
        "a8584c05037d6695926c93ea3c3ffe0c6a332454acdfb863901d5ca5151808f1"
    );
}
