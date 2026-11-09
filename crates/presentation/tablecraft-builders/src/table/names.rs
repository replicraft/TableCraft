//! Default header spelling shared by initial and saved-table naming surfaces.
use super::{MAX_DRAFT_FIELDS, TableDraftError};

/// Select the first unused FieldN spelling under the existing draft policy.
/// This is presentation naming only: native admission still checks the exact
/// proposed field name against its complete source and native name rules.
pub fn next_field_name<'a>(
    names: impl Iterator<Item = &'a str> + Clone,
) -> Result<String, TableDraftError> {
    // The bounded naming search replays the same borrowed names for candidate suffixes without
    // materializing their text; cloning iterator state preserves the original traversal.
    if names.clone().take(MAX_DRAFT_FIELDS + 1).count() >= MAX_DRAFT_FIELDS {
        return Err(TableDraftError::Limit);
    }
    for suffix in 1..=MAX_DRAFT_FIELDS {
        let candidate = format!("Field{suffix}");
        if !names
            // The bounded naming search replays the same borrowed names for candidate suffixes
            // without materializing their text; cloning iterator state preserves the original
            // traversal.
            .clone()
            .any(|name| name.eq_ignore_ascii_case(&candidate))
        {
            return Ok(candidate);
        }
    }
    Err(TableDraftError::Limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_header_policy_fills_first_gap_without_changing_existing_spelling() {
        let names = ["ID", "field1", "Field3", "Étiquette"];
        assert_eq!(next_field_name(names.iter().copied()).unwrap(), "Field2");
        assert_eq!(names, ["ID", "field1", "Field3", "Étiquette"]);
        assert_eq!(next_field_name(["ID"].into_iter()).unwrap(), "Field1");
        assert_eq!(next_field_name(["Field01"].into_iter()).unwrap(), "Field1");
    }

    #[test]
    fn full_source_refuses_before_generating_an_extra_header() {
        let names = ["ID"; MAX_DRAFT_FIELDS];
        assert_eq!(
            next_field_name(names.iter().copied()),
            Err(TableDraftError::Limit)
        );
    }
}
