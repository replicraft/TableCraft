#[path = "../calculation_preview/dependencies/fixture.rs"]
mod dependency;

pub(super) fn database(expressions: &[(&str, &str)], identities: &[i32]) -> Vec<u8> {
    dependency::database(expressions, identities)
}
