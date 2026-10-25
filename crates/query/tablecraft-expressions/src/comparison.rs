//! SQL comparison, independent of representational Value equality.
use crate::{EvalError, logic::Truth, numeric};
use std::{cmp::Ordering, ops::Range};
mod declared;
mod like;
pub use declared::{variant_expression_text_numeric, variant_text_numeric};
mod locale;
pub use locale::LocaleId;
#[cfg(feature = "icu-text")]
pub mod icu_text;
mod search;
mod string_input;
mod strings;
pub use like::{LikeProfile, LikeSpanState};
pub use search::{StringSearch, StringSearchProfile};
pub use string_input::StringInput;
#[cfg(feature = "sql")]
pub(crate) use strings::order_utf16_strings;
pub use strings::{StringComparisonProfile, compare_utf16_strings};
use tablecraft_core::values::Value;

/// Explicit Option Compare policy for string built-ins; no module setting is inferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringCompareMode {
    /// Lexicographic UTF-16 code-unit comparison.
    Binary,
    /// The host's separately supplied textual/locale comparison.
    Text,
    /// The explicitly selected database comparison policy.
    Database,
}

/// Explicit text ordering supplied by the host; no implicit Rust/locale collation.
pub trait Collation {
    /// Compare decoded strings according to the selected database policy.
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError>;
    /// Select raw-unit comparison explicitly, independently of the LIKE profile.
    fn string_comparison_profile(&self) -> StringComparisonProfile {
        StringComparisonProfile::Unicode
    }
    /// Compare complete UTF-16 Strings under this provider's SQL text policy.
    /// This is separate from LIKE's literal and character-class weights.
    /// Providers own cumulative callback work/cancellation and must refuse
    /// unsupported identities without a binary/locale fallback.
    fn compare_utf16(&self, _left: &[u16], _right: &[u16]) -> Result<Ordering, EvalError> {
        Err(EvalError::UnsupportedExpression)
    }
    /// Select matching units explicitly; ordering alone does not activate a
    /// UTF-16/expansion profile or change existing scalar providers.
    fn like_profile(&self) -> LikeProfile {
        LikeProfile::UnicodeScalar
    }
    /// Compare complete raw UTF-16 LIKE spans, retaining trailing spaces and
    /// isolated surrogate units. Providers own cumulative work/cancellation.
    fn compare_like_units(&self, _left: &[u16], _right: &[u16]) -> Result<Ordering, EvalError> {
        Err(EvalError::UnsupportedExpression)
    }
    /// Whether a span represents one element under the selected weight policy.
    /// A range comparison alone cannot classify arbitrary multi-unit words.
    /// Providers own cumulative work/cancellation, as for compare_like_units.
    fn is_like_unit(&self, _units: &[u16]) -> Result<bool, EvalError> {
        Err(EvalError::UnsupportedExpression)
    }
    /// Compare a literal and optionally prove that extending the candidate
    /// cannot match. Defaults never prune; an Impossible result is a host proof.
    fn like_literal_step(
        &self,
        candidate: &[u16],
        literal: &[u16],
    ) -> Result<LikeSpanState, EvalError> {
        Ok(
            if self.compare_like_units(candidate, literal)? == Ordering::Equal {
                LikeSpanState::Match
            } else {
                LikeSpanState::Possible
            },
        )
    }
    /// Recognize an element and optionally prove no extension can be one.
    /// Defaults retain every extension; providers own their proof/work limits.
    fn like_unit_step(&self, candidate: &[u16]) -> Result<LikeSpanState, EvalError> {
        Ok(if self.is_like_unit(candidate)? {
            LikeSpanState::Match
        } else {
            LikeSpanState::Possible
        })
    }
    /// Option Compare for omitted/-1 string built-in modes. Absence requires an
    /// explicit argument; supplying a collation alone does not guess a module policy.
    fn option_compare(&self) -> Option<StringCompareMode> {
        None
    }
    /// Search using an explicit text/database policy. Returns a zero-based UTF-16
    /// match range, including its exclusive end. Providers must honor search bounds
    /// and choose the earliest/latest match, allowing locale expansions/contractions.
    /// Equality callbacks alone do not implement substring matching. Hosts own
    /// callback work/cancellation limits; the default reports unsupported behavior.
    fn find(
        &self,
        _haystack: &str,
        _needle: &str,
        _search: StringSearch,
        _mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        Err(EvalError::UnsupportedExpression)
    }
    /// Explicit input support for one substring mode. Equality/LIKE profiles do
    /// not activate this contract, and database support does not imply text mode.
    fn string_search_profile(&self, _mode: StringCompareMode) -> StringSearchProfile {
        StringSearchProfile::Unicode
    }
    /// Search complete UTF-16 String units under an explicitly supported mode.
    /// Return the consumed unit range with the same bounds/earliest/latest and
    /// cumulative resource contracts as find(). No binary or Unicode retry occurs
    /// after a callback failure. LCID lookup remains a separate contract.
    fn find_utf16(
        &self,
        _haystack: &[u16],
        _needle: &[u16],
        _search: StringSearch,
        _mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        Err(EvalError::UnsupportedExpression)
    }
    /// Search under an explicit Windows LCID supplied to InStr. This is separate
    /// from the module's text mode and the database policy. Providers must resolve
    /// the entire identity (including alternate sort bits), enforce the same
    /// bounds/work/cancellation contract as find(), and reject unsupported IDs
    /// without a default-locale fallback. Structural validity is not support.
    fn find_locale(
        &self,
        _haystack: &str,
        _needle: &str,
        _search: StringSearch,
        _locale: LocaleId,
    ) -> Result<Option<Range<usize>>, EvalError> {
        Err(EvalError::CollationRequired)
    }
    /// Separate text/locale comparison for vbTextCompare. Existing database
    /// comparers do not silently become textual/locale comparers. Hosts own the
    /// work/cancellation bounds of callbacks, as for compare().
    fn compare_text(&self, _left: &str, _right: &str) -> Result<Ordering, EvalError> {
        Err(EvalError::CollationRequired)
    }
}
/// SQL comparison operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    /// =.
    Equal,
    /// <>.
    NotEqual,
    /// <.
    Less,
    /// <=.
    LessOrEqual,
    /// >.
    Greater,
    /// >=.
    GreaterOrEqual,
}
/// Compare numeric values or strings with an explicitly supplied collation.
/// Either NULL yields unknown, including NULL = NULL.
pub fn compare(
    operator: Comparison,
    left: &Value,
    right: &Value,
    collation: Option<&dyn Collation>,
) -> Result<Truth, EvalError> {
    if matches!(left, Value::Null) || matches!(right, Value::Null) {
        return Ok(Truth::Unknown);
    }
    let order = match (left, right) {
        (Value::Guid(left), Value::Guid(right)) => {
            if !matches!(operator, Comparison::Equal | Comparison::NotEqual) {
                return Err(EvalError::UnsupportedExpression);
            }
            let result =
                (left.to_bytes() == right.to_bytes()) == matches!(operator, Comparison::Equal);
            return Ok(if result { Truth::True } else { Truth::False });
        }
        (Value::Guid(_), _) | (_, Value::Guid(_)) => return Err(EvalError::TypeMismatch),
        (Value::DateTime(left), Value::DateTime(right)) => left.temporal_cmp(*right),
        (Value::DateTime(_), _) | (_, Value::DateTime(_)) => return Err(EvalError::TypeMismatch),
        (Value::DateTimeExtended(left), Value::DateTimeExtended(right)) => {
            (left.date(), left.time()).cmp(&(right.date(), right.time()))
        }
        (Value::DateTimeExtended(_), _) | (_, Value::DateTimeExtended(_)) => {
            return Err(EvalError::TypeMismatch);
        }
        (Value::Text(left), Value::Text(right)) => collation
            .ok_or(EvalError::CollationRequired)?
            .compare(left, right)?,
        (Value::Text(_), _) | (_, Value::Text(_)) => return Err(EvalError::TypeMismatch),
        _ => numeric::compare(left, right)?,
    };
    Ok(from_ordering(operator, order))
}

/// Apply a predicate to an already established, non-NULL ordering.
/// This performs no operand admission, coercion or collation lookup. Callers own
/// comparison correctness and resource checks; NULL requires its separate rule.
pub fn from_ordering(operator: Comparison, order: Ordering) -> Truth {
    let result = match operator {
        Comparison::Equal => order == Ordering::Equal,
        Comparison::NotEqual => order != Ordering::Equal,
        Comparison::Less => order == Ordering::Less,
        Comparison::LessOrEqual => order != Ordering::Greater,
        Comparison::Greater => order == Ordering::Greater,
        Comparison::GreaterOrEqual => order != Ordering::Less,
    };
    if result { Truth::True } else { Truth::False }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extended_dates_compare_civil_components_at_full_precision_without_legacy_coercion() {
        use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime};
        let first =
            Value::DateTimeExtended(ExtendedDateTime::new(2000, 2, 29, 12, 0, 0, 1).unwrap());
        let next =
            Value::DateTimeExtended(ExtendedDateTime::new(2000, 2, 29, 12, 0, 0, 2).unwrap());
        assert_eq!(
            compare(Comparison::Less, &first, &next, None),
            Ok(Truth::True)
        );
        assert_eq!(
            compare(Comparison::Equal, &first, &next, None),
            Ok(Truth::False)
        );
        assert_eq!(
            compare(Comparison::Equal, &first, &first, None),
            Ok(Truth::True)
        );
        assert_eq!(
            compare(Comparison::Less, &first, &Value::Null, None),
            Ok(Truth::Unknown)
        );
        assert_eq!(
            compare(
                Comparison::Equal,
                &first,
                &Value::DateTime(LegacyDateTime::from_bits(0).unwrap()),
                None
            ),
            Err(EvalError::TypeMismatch)
        );
    }
    struct Ordinal;
    impl Collation for Ordinal {
        fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
            Ok(left.cmp(right))
        }
    }
    #[test]
    fn null_is_unknown_and_text_needs_a_selected_collation() {
        for op in [
            Comparison::Equal,
            Comparison::NotEqual,
            Comparison::Less,
            Comparison::LessOrEqual,
            Comparison::Greater,
            Comparison::GreaterOrEqual,
        ] {
            assert_eq!(
                compare(op, &Value::Null, &Value::Null, None),
                Ok(Truth::Unknown)
            );
        }
        let (a, b) = (Value::Text("A".into()), Value::Text("a".into()));
        assert_eq!(
            compare(Comparison::Equal, &a, &b, None),
            Err(EvalError::CollationRequired)
        );
        assert_eq!(
            compare(Comparison::Less, &a, &b, Some(&Ordinal)),
            Ok(Truth::True)
        );
        assert_eq!(
            compare(Comparison::Equal, &Value::Byte(1), &Value::Int64(1), None),
            Ok(Truth::True)
        );
    }
}

#[cfg(test)]
mod temporal_tests {
    use super::*;
    use tablecraft_core::values::LegacyDateTime;
    fn date(days: f64) -> Value {
        Value::DateTime(LegacyDateTime::from_ole_days(days).unwrap())
    }
    #[test]
    fn legacy_dates_compare_temporally_without_numeric_or_text_coercion() {
        assert_eq!(
            compare(Comparison::Less, &date(-1.25), &date(-1.75), None),
            Ok(Truth::True)
        );
        assert_eq!(
            compare(Comparison::Equal, &date(-0.25), &date(0.25), None),
            Ok(Truth::True)
        );
        assert_eq!(
            compare(Comparison::Equal, &date(1.0), &Value::Int64(1), None),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            compare(Comparison::Equal, &date(1.0), &Value::Null, None),
            Ok(Truth::Unknown)
        );
    }
}
