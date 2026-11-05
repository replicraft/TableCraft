//! Bounded paint/accessibility text; the original cell remains the copy source.
use std::borrow::Cow;

const MAX_PREVIEW_SCALARS: usize = 512;
const MAX_PREVIEW_UTF8_BYTES: usize = 1024;
const MAX_PREVIEW_LINES: usize = 8;
const SHORTENED_MARKER: &str = "…";

pub(super) struct Preview<'a> {
    pub(super) text: Cow<'a, str>,
    pub(super) shortened: bool,
}

/// Read only a bounded prefix and one lookahead scalar. A Unicode scalar
/// boundary is deliberately used instead of an unbounded grapheme boundary:
/// one combining sequence may itself contain an arbitrarily large payload.
/// No normalization or mutation of the original value occurs.
pub(super) fn text(value: &str) -> Preview<'_> {
    let mut lines = 1;
    let mut previous_carriage_return = false;
    for (scalars, (offset, character)) in value.char_indices().enumerate() {
        // CRLF is one line break; standalone CR and Unicode paragraph/line
        // separators also consume a line. Preserve the original sequence.
        let line_break = match character {
            '\n' => !previous_carriage_return,
            '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}' => true,
            _ => false,
        };
        if scalars == MAX_PREVIEW_SCALARS
            || offset + character.len_utf8() > MAX_PREVIEW_UTF8_BYTES
            || line_break && lines == MAX_PREVIEW_LINES
        {
            let prefix = &value[..offset];
            let mut text = String::with_capacity(prefix.len() + SHORTENED_MARKER.len());
            text.push_str(prefix);
            text.push_str(SHORTENED_MARKER);

            return Preview {
                text: Cow::Owned(text),
                shortened: true,
            };
        }
        if line_break {
            lines += 1;
        }
        previous_carriage_return = character == '\r';
    }

    Preview {
        text: Cow::Borrowed(value),
        shortened: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_values_borrow_exact_text_without_claiming_shortening() {
        for value in [
            "",
            "NULL",
            "literal…",
            "a\tb\r\nc",
            "é e\u{301} 👩\u{200d}💻",
        ] {
            let preview = text(value);
            assert!(matches!(preview.text, Cow::Borrowed(_)));
            assert_eq!(preview.text, value);
            assert!(!preview.shortened);
        }

        let exact_scalars = "a".repeat(512);
        assert!(matches!(text(&exact_scalars).text, Cow::Borrowed(_)));
        let exact_bytes = "🦀".repeat(256);
        assert!(matches!(text(&exact_bytes).text, Cow::Borrowed(_)));
    }

    #[test]
    fn long_values_have_independent_scalar_and_utf8_bounds_with_explicit_shortening() {
        for (value, expected) in [
            ("a".repeat(1_000_000), format!("{}…", "a".repeat(512))),
            ("🦀".repeat(10_000), format!("{}…", "🦀".repeat(256))),
            (
                format!("{}🦀tail", "界".repeat(341)),
                format!("{}…", "界".repeat(341)),
            ),
            (
                format!("e{}", "\u{301}".repeat(10_000)),
                format!("e{}…", "\u{301}".repeat(511)),
            ),
        ] {
            let original = value.clone();
            let preview = text(&value);
            assert!(preview.shortened);
            assert_eq!(preview.text, expected);
            assert!(preview.text.len() <= 1024 + "…".len());
            assert!(preview.text.chars().count() <= 512 + 1);
            assert_eq!(value, original);
        }
    }

    #[test]
    fn pathological_empty_lines_and_paragraphs_stop_before_the_ninth_line() {
        for separator in ["\n", "\r", "\r\n", "\u{0085}", "\u{2028}", "\u{2029}"] {
            let value = separator.repeat(1_000);
            let preview = text(&value);
            assert!(preview.shortened);
            assert_eq!(preview.text, format!("{}…", separator.repeat(7)));
            assert_eq!(value, separator.repeat(1_000));
        }

        let exact = "line\n".repeat(7) + "last";
        assert!(!text(&exact).shortened);
        assert_eq!(text(&exact).text, exact);
    }
}
