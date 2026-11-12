//! Runtime's explicit SQL View policy, separate from raw native observations.
use tablecraft_ace::DatabaseQueryMode;
use tablecraft_expressions::predicates::LikeMode;

/// Access documents ANSI-89 as its desktop database query default. This is an
/// application choice, not a claim that absent MR2 bytes encode native zero.
pub(crate) const SQL_VIEW_DEFAULT: LikeMode = LikeMode::Ansi89;

/// Native known settings remain authoritative. Only qualified absence selects
/// this host default; unknown metadata never becomes a guessed wildcard mode.
pub(crate) fn sql_view(observed: DatabaseQueryMode) -> Option<LikeMode> {
    match observed {
        DatabaseQueryMode::Absent => Some(SQL_VIEW_DEFAULT),
        DatabaseQueryMode::Ansi89 => Some(LikeMode::Ansi89),
        DatabaseQueryMode::Ansi92 => Some(LikeMode::Ansi92),
        DatabaseQueryMode::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_view_default_is_distinct_from_explicit_and_unknown_native_settings() {
        assert_eq!(sql_view(DatabaseQueryMode::Absent), Some(LikeMode::Ansi89));
        assert_eq!(sql_view(DatabaseQueryMode::Ansi89), Some(LikeMode::Ansi89));
        assert_eq!(sql_view(DatabaseQueryMode::Ansi92), Some(LikeMode::Ansi92));
        assert_eq!(sql_view(DatabaseQueryMode::Unknown), None);
    }
}
