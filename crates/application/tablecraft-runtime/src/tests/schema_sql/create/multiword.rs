//! Equivalent SQL spellings keep literal native widths in the existing creation journal.
use super::save;
use tablecraft_testkit::fixtures::access::{
    canonical_blank_creation, canonical_native17_creation, canonical_variable_creation,
};

#[test]
fn compound_character_and_binary_aliases_save_the_same_independent_native_declarations() {
    let before = canonical_blank_creation::source();
    let expected = canonical_native17_creation::source(
        canonical_variable_creation::source(canonical_variable_creation::Case::Empty).bytes,
    );
    for (position, text) in [
        "CHARACTER VARYING",
        "NATIONAL CHARACTER",
        "NATIONAL CHAR",
        "NATIONAL CHARACTER VARYING",
        "NATIONAL CHAR VARYING",
    ]
    .into_iter()
    .enumerate()
    {
        let binary = if position % 2 == 0 {
            "BINARY VARYING"
        } else {
            "BIT VARYING"
        };
        let sql = format!(
            "CREATE TABLE Items (Id INTEGER, TextValue {text}(8), BinaryValue {binary}(16), DecimalValue DECIMAL(28,4), Flag BIT)"
        );
        save(&before, &expected, &sql, "Items", 5);
    }
}
