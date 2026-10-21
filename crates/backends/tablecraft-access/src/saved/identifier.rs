//! Stored names may already carry one Access bracket delimiter pair.
use super::SavedSelectError as E;

pub(super) fn literal(value: &str) -> Result<&str, E> {
    let value = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(value);
    if value.is_empty() || value.chars().any(|c| c.is_control() || "[]`.!".contains(c)) {
        return Err(E::Unsupported);
    }
    Ok(value)
}
