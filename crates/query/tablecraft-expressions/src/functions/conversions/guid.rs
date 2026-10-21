//! Access GUID byte-array conversion, separate from canonical stored GUID values.
use crate::EvalError;
use tablecraft_core::values::Guid;

const GUID_LITERAL_PREFIX: &str = "{guid ";
const GUID_LITERAL_SUFFIX: &str = "}";
const GUID_LITERAL_BYTES: usize =
    GUID_LITERAL_PREFIX.len() + uuid::fmt::Braced::LENGTH + GUID_LITERAL_SUFFIX.len();

/// Parse a hyphenated, braced or Access `{guid {…}}` spelling into GUID-structure
/// bytes. The first DWORD and next two WORDs use little endian; the tail is literal.
/// This fixed-size result allocates nothing and does not generate an identifier.
pub fn from_string(text: &str) -> Result<[u8; Guid::BYTE_LEN], EvalError> {
    let text = if text.len() == GUID_LITERAL_BYTES
        && text
            .get(..GUID_LITERAL_PREFIX.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(GUID_LITERAL_PREFIX))
        && text.ends_with(GUID_LITERAL_SUFFIX)
    {
        &text[GUID_LITERAL_PREFIX.len()..text.len() - GUID_LITERAL_SUFFIX.len()]
    } else {
        text
    };
    if !matches!(
        text.len(),
        uuid::fmt::Hyphenated::LENGTH | uuid::fmt::Braced::LENGTH
    ) {
        return Err(EvalError::TypeMismatch);
    }

    uuid::Uuid::try_parse(text)
        .map(|value| value.to_bytes_le())
        .map_err(|_| EvalError::TypeMismatch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn mixed_endian_bytes_and_accepted_envelopes_match_independent_goldens() {
        let expected = [
            0x40, 0xfc, 0x29, 0x6b, 0x47, 0xca, 0x67, 0x10, 0xb3, 0x1d, 0x00, 0xdd, 0x01, 0x06,
            0x62, 0xda,
        ];
        for text in [
            "6B29FC40-CA47-1067-B31D-00DD010662DA",
            "{6b29fc40-ca47-1067-b31d-00dd010662da}",
            "{guid {6B29FC40-CA47-1067-B31D-00DD010662DA}}",
        ] {
            assert_eq!(from_string(text), Ok(expected));
        }
    }

    #[test]
    fn malformed_unicode_shapes_and_unqualified_uuid_spellings_fail() {
        for text in [
            "",
            "6b29fc40ca471067b31d00dd010662da",
            "urn:uuid:6b29fc40-ca47-1067-b31d-00dd010662da",
            " 6b29fc40-ca47-1067-b31d-00dd010662da",
            "{guid {6B29FC40-CA47-1067-B31D-00DD010662DZ}}",
            "{guid 6B29FC40-CA47-1067-B31D-00DD010662DA}",
        ] {
            assert_eq!(from_string(text), Err(EvalError::TypeMismatch));
        }
        let unicode = format!("aaaaa🦀{}", "b".repeat(36));
        assert_eq!(unicode.len(), 45);
        assert_eq!(from_string(&unicode), Err(EvalError::TypeMismatch));
    }

    proptest! {
        #![proptest_config(crate::test_support::config())]

        #[test]
        fn random_canonical_bytes_follow_independent_guid_group_reversal(bytes in any::<[u8; 16]>()) {
            let text = uuid::Uuid::from_bytes(bytes).hyphenated().to_string();
            let mut expected = bytes;
            expected[..4].reverse();
            expected[4..6].reverse();
            expected[6..8].reverse();

            prop_assert_eq!(from_string(&text), Ok(expected));
        }
    }
}
