//! Conservatively exclude directions whose logical glyph association is private.
use harfrust::{Direction, UnicodeBuffer};

// Unicode 17.0 PropList.txt, Bidi_Control (12 characters):
// https://www.unicode.org/Public/17.0.0/ucd/PropList.txt
// These are formatting controls, not an approximation of RTL script ranges.
const BIDI_CONTROLS: [char; 12] = [
    '\u{061c}', // ARABIC LETTER MARK
    '\u{200e}', // LEFT-TO-RIGHT MARK
    '\u{200f}', // RIGHT-TO-LEFT MARK
    '\u{202a}', // LEFT-TO-RIGHT EMBEDDING
    '\u{202b}', // RIGHT-TO-LEFT EMBEDDING
    '\u{202c}', // POP DIRECTIONAL FORMATTING
    '\u{202d}', // LEFT-TO-RIGHT OVERRIDE
    '\u{202e}', // RIGHT-TO-LEFT OVERRIDE
    '\u{2066}', // LEFT-TO-RIGHT ISOLATE
    '\u{2067}', // RIGHT-TO-LEFT ISOLATE
    '\u{2068}', // FIRST STRONG ISOLATE
    '\u{2069}', // POP DIRECTIONAL ISOLATE
];

pub(super) fn unambiguous(text: &str) -> bool {
    let mut buffer = UnicodeBuffer::new();
    let mut encoded = [0; char::MAX.len_utf8()];
    for character in text.chars() {
        if BIDI_CONTROLS.contains(&character) {
            return false;
        }
        // epaint guesses direction independently for font runs. Testing only
        // the complete string would miss a later RTL fallback run. The same
        // shaper's per-scalar script inference also covers weak digits/marks.
        buffer.clear();
        buffer.push_str(character.encode_utf8(&mut encoded));
        buffer.guess_segment_properties();
        if buffer.direction() != Direction::LeftToRight {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::unambiguous;

    #[test]
    fn script_direction_excludes_rtl_even_when_logical_characters_are_identical() {
        for text in ["אבא", "אאא", "سسس", "a אבא b", "a\u{06f1}b", "\u{05b0}"] {
            assert!(!unambiguous(text), "unqualified script direction {text:?}");
        }
        for text in [
            "office",
            "e\u{301}",
            "👩\u{200d}💻",
            "日本語",
            "한국어",
            "Привет",
            "\u{093f}",
        ] {
            assert!(unambiguous(text), "LTR script direction {text:?}");
        }
    }

    #[test]
    fn all_unicode_bidi_controls_refuse_independently_of_font_coverage() {
        // Independent UCD inputs, deliberately not derived from BIDI_CONTROLS.
        for character in "\u{061c}\u{200e}\u{200f}\u{202a}\u{202b}\u{202c}\u{202d}\u{202e}\u{2066}\u{2067}\u{2068}\u{2069}".chars() {
            assert!(!unambiguous(&format!("a{character}a")));
        }
    }
}
