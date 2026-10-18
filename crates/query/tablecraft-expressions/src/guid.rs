//! Bounded canonical GUID literal parsing through the uuid library.
use crate::EvalError;
use tablecraft_core::values::Guid;

/// Parse a hyphenated 36-byte GUID, preserving canonical bytes. No ID generation,
/// native field/index ordering, RNG or implicit string coercion is introduced.
pub fn parse_literal(text: &str) -> Result<Guid, EvalError> {
    if text.len() != uuid::fmt::Hyphenated::LENGTH {
        return Err(EvalError::UnsupportedLiteral);
    }
    uuid::Uuid::try_parse(text)
        .map(|value| Guid::from_bytes(value.into_bytes()))
        .map_err(|_| EvalError::UnsupportedLiteral)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_guid_parsing_preserves_bytes_and_rejects_other_spellings() {
        let upper = parse_literal("6B29FC40-CA47-1067-B31D-00DD010662DA").unwrap();
        assert_eq!(
            upper,
            parse_literal("6b29fc40-ca47-1067-b31d-00dd010662da").unwrap()
        );
        assert!(parse_literal("{6b29fc40-ca47-1067-b31d-00dd010662da}").is_err());
        assert!(parse_literal("6b29fc40ca471067b31d00dd010662da").is_err());
    }
}
