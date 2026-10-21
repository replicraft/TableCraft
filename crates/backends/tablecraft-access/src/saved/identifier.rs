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

/// Match the reference's literal-prompt/qualified-control distinction without
/// interpreting Forms/control paths or acquiring an application binding.
pub(super) fn prompt(value: &str) -> Result<&str, E> {
    let body = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'));
    let bracketed = body.is_some();
    let value = body.unwrap_or(value);
    if value.is_empty() || value.chars().any(|ch| ch.is_control() || "[]".contains(ch)) {
        return Err(E::Unsupported);
    }
    if !bracketed
        && value.contains('!')
        && value
            .split(['!', '.'])
            .all(|part| !part.is_empty() && !part.starts_with(' ') && !part.ends_with(' '))
    {
        return Err(E::Unsupported);
    }
    Ok(value)
}
