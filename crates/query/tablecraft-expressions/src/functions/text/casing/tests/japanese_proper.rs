use super::*;

fn proper(text: &str, language: &str) -> String {
    let mut output = Output {
        remaining: 100_000,
        ..Default::default()
    };
    Casing::proper(text)
        .write(Some(&language.parse().unwrap()), &mut output)
        .unwrap();
    output.text
}

#[test]
fn ideographic_space_is_an_additional_boundary_only_for_explicit_japanese() {
    for language in ["ja", "ja-JP", "JA-jp", "ja-Latn-JP"] {
        for (text, expected) in [
            ("hello　WORLD", "Hello　World"),
            ("ｈＥＬＬＯ　ｗＯＲＬＤ", "Ｈｅｌｌｏ　Ｗｏｒｌｄ"),
            ("　hello　　WORLD　", "　Hello　　World　"),
            ("東京abc　大阪DEF", "東京Abc　大阪Def"),
            ("ß　ﬁ", "Ss　Fi"),
            ("hello\0WORLD\tfoo\nBAR", "Hello\0World\tFoo\nBar"),
            ("a\u{a0}B\u{2003}C", "A\u{a0}b\u{2003}c"),
            ("", ""),
        ] {
            assert_eq!(proper(text, language), expected, "{language}: {text:?}");
        }
    }
    for language in ["en-US", "und", "de-DE", "tr", "zh", "ko"] {
        assert_eq!(
            proper("hello　WORLD", language),
            "Hello　world",
            "{language}"
        );
    }
}

#[test]
fn japanese_segment_output_and_each_observed_step_keep_sink_failures() {
    let locale = "ja-JP".parse().unwrap();
    let recipe = Casing::proper("hello　WORLD　ß　ﬁ");
    let mut output = Output {
        remaining: 100_000,
        ..Default::default()
    };
    recipe.write(Some(&locale), &mut output).unwrap();
    let steps = 100_000 - output.remaining;
    assert!(steps > 0);

    for before_failure in 0..steps {
        let mut output = Output {
            remaining: before_failure,
            ..Default::default()
        };
        assert_eq!(
            recipe.write(Some(&locale), &mut output),
            Err(EvalError::Cancelled)
        );
    }
    let mut output = Output {
        remaining: 100_000,
        append_error: Some(EvalError::TextLimit),
        ..Default::default()
    };
    assert_eq!(
        recipe.write(Some(&locale), &mut output),
        Err(EvalError::TextLimit)
    );
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn japanese_boundaries_match_an_independent_ascii_word_state_model(
        characters in prop::collection::vec(
            prop_oneof![
                prop::char::range('a', 'z'),
                prop::char::range('A', 'Z'),
                Just(' '),
                Just('\t'),
                Just('\0'),
                Just('\u{3000}'),
                Just('\u{a0}'),
                Just('-'),
            ],
            0..80,
        ),
    ) {
        let text: String = characters.iter().collect();
        for (language, japanese) in [("ja-JP", true), ("en-US", false)] {
            let mut expected = String::new();
            let mut initial = true;
            for character in &characters {
                if matches!(character, ' ' | '\t' | '\0') || japanese && *character == '\u{3000}' {
                    initial = true;
                    expected.push(*character);
                } else if character.is_ascii_alphabetic() {
                    let next = if initial {
                        character.to_ascii_uppercase()
                    } else {
                        character.to_ascii_lowercase()
                    };
                    expected.push(next);
                    initial = false;
                } else {
                    expected.push(*character);
                }
            }
            prop_assert_eq!(proper(&text, language), expected);
        }
    }
}
