use super::*;
use crate::functions::text::{TextFunction, TextResult, apply};
use proptest::prelude::*;

#[derive(Default)]
struct Output {
    text: String,
    remaining: usize,
    append_error: Option<EvalError>,
}
impl TextOutput for Output {
    fn step(&mut self) -> Result<(), EvalError> {
        self.remaining = self.remaining.checked_sub(1).ok_or(EvalError::Cancelled)?;
        Ok(())
    }
    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        if let Some(error) = self.append_error {
            return Err(error);
        }
        self.text.push_str(text);
        Ok(())
    }
}
fn convert(text: &str, upper: bool, locale: &str) -> String {
    let mut output = Output {
        remaining: 100_000,
        ..Default::default()
    };
    Casing::new(text, upper)
        .write(Some(&locale.parse().unwrap()), &mut output)
        .unwrap();
    output.text
}
#[test]
fn unicode_context_language_expansion_and_nonletters_are_preserved() {
    for (text, upper, locale, expected) in [
        ("Hello World 1234", false, "en-US", "hello world 1234"),
        ("Hello World 1234", true, "en-US", "HELLO WORLD 1234"),
        ("Iİiı", false, "tr", "ıiiı"),
        ("Iİiı", true, "az", "IİİI"),
        ("Iİiı", false, "und", "ii\u{307}iı"),
        ("ΟΣ ΟΣΑ", false, "el", "ος οσα"),
        ("I\u{301}", false, "lt", "i\u{307}\u{301}"),
        ("straße ﬃ", true, "de-DE", "STRASSE FFI"),
        ("é\0中🦀\n", true, "en-US", "É\0中🦀\n"),
        ("", false, "und", ""),
    ] {
        assert_eq!(convert(text, upper, locale), expected, "{locale} {text:?}");
    }
}
#[test]
fn null_type_shape_and_language_validation_are_explicit() {
    use tablecraft_core::values::Value;
    for function in [TextFunction::LCase, TextFunction::UCase] {
        assert_eq!(
            apply(function, &[Value::Null], || Ok(())),
            Ok(TextResult::Null)
        );
        assert_eq!(
            apply(function, &[Value::Int16(1)], || Ok(())),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            apply(function, &[], || Ok(())),
            Err(EvalError::UnsupportedExpression)
        );
    }
    for text in [
        "",
        "en_US",
        "en-US-u-co-phonebk",
        "é",
        "en--US",
        "en-US-x-private",
    ] {
        assert_eq!(
            text.parse::<CaseLocale>(),
            Err(EvalError::InvalidCaseLocale),
            "{text}"
        );
    }
    assert_eq!(
        "x".repeat(129).parse::<CaseLocale>(),
        Err(EvalError::InvalidCaseLocale)
    );
    assert_eq!(
        "EN-us".parse::<CaseLocale>().unwrap(),
        "en-US".parse().unwrap()
    );
}
#[test]
fn missing_policy_context_preflight_and_sink_errors_are_retained() {
    let mut output = Output {
        remaining: 100_000,
        ..Default::default()
    };
    assert_eq!(
        Casing::new("", false).write(None, &mut output),
        Err(EvalError::CaseLocaleRequired)
    );
    let locale = "und".parse().unwrap();
    output.remaining = 10;
    assert_eq!(
        Casing::new("Σ\u{301}\u{301}", false).write(Some(&locale), &mut output),
        Err(EvalError::Cancelled)
    );
    assert!(output.text.is_empty());
    let text = "α\u{301}".repeat(4);
    output.remaining = 100;
    Casing::new(&text, true)
        .write(Some(&locale), &mut output)
        .unwrap();
    output.text.clear();
    output.remaining = 100;
    let greek = "el".parse().unwrap();
    assert_eq!(
        Casing::new(&text, true).write(Some(&greek), &mut output),
        Err(EvalError::Cancelled)
    );
    assert!(
        output.text.is_empty(),
        "Greek context reserve must precede mapping"
    );
    for error in [
        EvalError::Allocation,
        EvalError::TextLimit,
        EvalError::Cancelled,
        EvalError::Domain,
    ] {
        output.remaining = 100_000;
        output.append_error = Some(error);
        assert_eq!(
            Casing::new("abc", true).write(Some(&locale), &mut output),
            Err(error)
        );
    }
}
proptest! {
    #[test]
    fn ascii_mapping_matches_independent_integer_offsets(bytes in prop::collection::vec(0u8..=127, 0..100)) {
        let text = String::from_utf8(bytes.clone()).unwrap();
        for upper in [true, false] {
            let expected: String = bytes.iter().map(|&byte| char::from(if upper && (97..=122).contains(&byte) { byte-32 } else if !upper && (65..=90).contains(&byte) { byte+32 } else {byte})).collect();
            prop_assert_eq!(convert(&text, upper, "und"), expected);
        }
    }
}
