use super::*;
use crate::comparison::LocaleId;
use proptest::prelude::*;

#[derive(Default)]
struct Output {
    text: String,
    bytes: Vec<u8>,
}

impl TextOutput for Output {
    fn step(&mut self) -> Result<(), EvalError> {
        Ok(())
    }

    fn append(&mut self, fragment: &str) -> Result<(), EvalError> {
        self.text.push_str(fragment);
        Ok(())
    }
}

impl ByteOutput for Output {
    fn step(&mut self) -> Result<(), EvalError> {
        Ok(())
    }

    fn append(&mut self, fragment: &[u8]) -> Result<(), EvalError> {
        self.bytes.extend_from_slice(fragment);
        Ok(())
    }
}

#[test]
fn conversion_flags_reject_conflicts_without_silently_dropping_requested_work() {
    for (code, case, encoding) in [
        (0, CaseConversion::Unchanged, EncodingConversion::Unchanged),
        (1, CaseConversion::Upper, EncodingConversion::Unchanged),
        (2, CaseConversion::Lower, EncodingConversion::Unchanged),
        (3, CaseConversion::Proper, EncodingConversion::Unchanged),
        (64, CaseConversion::Unchanged, EncodingConversion::ToUnicode),
        (67, CaseConversion::Proper, EncodingConversion::ToUnicode),
        (
            128,
            CaseConversion::Unchanged,
            EncodingConversion::FromUnicode,
        ),
        (130, CaseConversion::Lower, EncodingConversion::FromUnicode),
    ] {
        let conversion = Conversion::from_code(code).unwrap();
        assert_eq!(conversion.case(), case);
        assert_eq!(conversion.encoding(), encoding);
    }
    for code in [-1, 256, 12, 48, 192, 255] {
        assert_eq!(Conversion::from_code(code), Err(EvalError::Domain));
    }
    for code in [4, 8, 16, 32, 17, 33, 80, 160, 5, 132] {
        assert_eq!(
            Conversion::from_code(code),
            Err(EvalError::UnsupportedExpression)
        );
    }
}

#[test]
fn explicit_lcid_profiles_keep_exact_identities_and_distinct_code_pages() {
    for (lcid, language, page) in [
        (1033, "en-US", 1252),
        (1049, "ru-RU", 1251),
        (1055, "tr-TR", 1254),
        (1054, "th-TH", 874),
        (1066, "vi-VN", 1258),
    ] {
        let profile = LocaleProfile::from_lcid(LocaleId::new(lcid).unwrap()).unwrap();
        assert_eq!(profile.code_page().windows_id(), page);
        assert_eq!(profile.case_locale(), &language.parse().unwrap());
    }
    for lcid in [0x10409, 0x400, 0x800, 0x1000, 1041] {
        assert_eq!(
            LocaleProfile::from_lcid(LocaleId::new(lcid).unwrap()),
            Err(EvalError::UnsupportedExpression)
        );
    }
}

#[test]
fn strict_single_byte_kernels_preserve_nul_and_refuse_unmappable_data() {
    let page = CharacterCodePage::from_windows_id(1252).unwrap();
    let mut output = Output::default();
    encode_ansi("Aé€\0", Some(page), &mut output).unwrap();
    assert_eq!(output.bytes, [65, 233, 128, 0]);
    let bytes = output.bytes.clone();
    decode_ansi(&bytes, Some(page), &mut output).unwrap();
    assert_eq!(output.text, "Aé€\0");
    assert_eq!(
        encode_ansi("🦀", Some(page), &mut output),
        Err(EvalError::UnmappableCharacter)
    );
    assert_eq!(
        encode_ansi("é", None, &mut output),
        Err(EvalError::CharacterPolicyRequired)
    );
    assert_eq!(
        decode_ansi(&[128], None, &mut output),
        Err(EvalError::CharacterPolicyRequired)
    );
    let greek = CharacterCodePage::from_windows_id(1253).unwrap();
    assert_eq!(
        decode_ansi(&[170], Some(greek), &mut output),
        Err(EvalError::InvalidCharacterCode)
    );
}

#[test]
fn proper_case_uses_documented_separators_and_explicit_language_context() {
    let conversion = Conversion::from_code(3).unwrap();
    let english = "en-US".parse().unwrap();
    let text = "hELLO\0wORLD\tFOO\nBAR\u{b}BAZ\u{c}QUX\rZIP zap o'NEILL ab-CD";
    let mut output = Output::default();
    conversion
        .casing(text)
        .unwrap()
        .write(Some(&english), &mut output)
        .unwrap();
    assert_eq!(
        output.text,
        "Hello\0World\tFoo\nBar\u{b}Baz\u{c}Qux\rZip Zap O'neill Ab-cd"
    );
    let turkish = "tr".parse().unwrap();
    let mut output = Output::default();
    conversion
        .casing("istanbul IZMIR")
        .unwrap()
        .write(Some(&turkish), &mut output)
        .unwrap();
    assert_eq!(output.text, "İstanbul Izmır");
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn ascii_proper_case_matches_separator_state_model(
        characters in prop::collection::vec(prop_oneof![prop::char::range('a', 'z'), prop::char::range('A', 'Z'), Just(' '), Just('\t'), Just('\0')], 0..100),
    ) {
        let text: String = characters.iter().collect();
        let mut first = true;
        let expected: String = characters.iter().map(|character| {
            if matches!(character, ' ' | '\t' | '\0') {
                first = true;
                *character
            } else if first {
                first = false;
                character.to_ascii_uppercase()
            } else {
                character.to_ascii_lowercase()
            }
        }).collect();
        let locale = "en-US".parse().unwrap();
        let mut output = Output::default();
        Conversion::from_code(3).unwrap().casing(&text).unwrap().write(Some(&locale), &mut output).unwrap();
        prop_assert_eq!(output.text, expected);
    }
}
