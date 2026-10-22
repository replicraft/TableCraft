use super::*;
use proptest::prelude::*;

#[derive(Default)]
struct Buffer(String);

impl TextOutput for Buffer {
    fn step(&mut self) -> Result<(), EvalError> {
        Ok(())
    }

    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        self.0.push_str(text);
        Ok(())
    }
}

fn render(source: &str, mask: &str, language: Option<&str>) -> Result<String, EvalError> {
    let locale = language.map(str::parse).transpose()?;
    let mut output = Buffer::default();
    Display::new(source, mask, &mut output)?.write(locale.as_ref(), &mut output)?;
    Ok(output.0)
}

#[test]
fn required_optional_fill_directions_literals_and_overflow_are_distinct() {
    for (source, mask, expected) in [
        ("Ab", "@@@@", "  Ab"),
        ("Ab", "!@@@@", "Ab  "),
        ("Ab", "&&&&", "Ab"),
        ("Ab", "!&&&&", "Ab"),
        ("Ab", "@&@&", " Ab"),
        ("Ab", "!@&@&", "Ab "),
        ("", "@@-&&", "  -"),
        ("1234", "@@-@@", "12-34"),
        ("123456", "@@-@@", "12-3456"),
        ("Abcd", "!@@\\!", "Ab!cd"),
        ("Ab", r#""<&>!;"@@"#, "<&>!;Ab"),
        ("a\0b", "@@@", "a\0b"),
    ] {
        assert_eq!(render(source, mask, None), Ok(expected.into()), "{mask}");
    }
}

#[test]
fn casing_preserves_context_expansion_and_literal_case() {
    for (source, mask, language, expected) in [
        ("straße", r#">"prefix:"@@@@@@"#, "de", "prefix:STRASSE"),
        ("ΟΣ", "<@@@", "el", " ος"),
        ("iı", ">@@", "tr", "İI"),
        ("İ", "<@@", "und", "i\u{307}"),
        ("Ab", "@@@@", "und", "  Ab"),
    ] {
        assert_eq!(render(source, mask, Some(language)), Ok(expected.into()));
    }
    assert_eq!(
        render("abc", ">@@", None),
        Err(EvalError::CaseLocaleRequired)
    );
    assert_eq!(
        render("abc", "><@@", Some("und")),
        Err(EvalError::UnsupportedExpression)
    );
}

#[test]
fn surrogate_pairs_survive_adjacent_slots_and_refuse_interposed_literals() {
    for (mask, expected) in [
        ("@@@@@", "  é🦀"),
        ("!@@@@@", "é🦀  "),
        ("@@", "é🦀"),
        ("@-@@", "é-🦀"),
        ("@&", "é🦀"),
    ] {
        assert_eq!(render("é🦀", mask, None), Ok(expected.into()), "{mask}");
    }
    assert_eq!(
        render("🦀", "@-@", None),
        Err(EvalError::UnrepresentableText)
    );
    assert_eq!(
        render("🦀", "@\\!", None),
        Err(EvalError::UnrepresentableText)
    );
    assert_eq!(render("🦀", r#"@""@"#, None), Ok("🦀".into()));
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn ascii_layout_matches_an_independent_position_model(
        source in "[a-zA-Z0-9]{0,40}",
        slots in prop::collection::vec(any::<bool>(), 1..30),
        left in any::<bool>(),
    ) {
        let mut mask = if left { "!".to_owned() } else { String::new() };
        let padding = if left { 0 } else { slots.len().saturating_sub(source.len()) };
        let mut expected = String::new();
        let mut input = source.chars();
        for (index, required) in slots.iter().enumerate() {
            mask.push(if *required { '@' } else { '&' });
            mask.push('-');
            let character = if index < padding { None } else { input.next() };
            match character {
                Some(character) => expected.push(character),
                None if *required => expected.push(' '),
                None => {}
            }
            expected.push('-');
        }
        expected.extend(input);
        prop_assert_eq!(render(&source, &mask, None).unwrap(), expected);
    }
}
