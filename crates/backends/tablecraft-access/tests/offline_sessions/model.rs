//! Literal original and successive complete images, independent of session code.
use tablecraft_testkit::access::mutation_fixture;

pub fn original() -> Vec<u8> {
    mutation_fixture::database()
}

pub fn with_score(score: i32) -> Vec<u8> {
    let mut expected = original();
    // Independently authored original first-row Score wire; do not derive this
    // offset from mutation or session implementation constants.
    let offset = 12 * 4096 + 4091;
    assert_eq!(&expected[offset..offset + 4], &10_i32.to_le_bytes());
    expected[offset..offset + 4].copy_from_slice(&score.to_le_bytes());
    expected
}
