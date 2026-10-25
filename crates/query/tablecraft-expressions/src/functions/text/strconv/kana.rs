//! Basic Japanese kana mappings without width conversion or implicit normalization.
use crate::{EvalError, functions::text::output::TextOutput};

const SCRIPT_OFFSET: u32 = 'ァ' as u32 - 'ぁ' as u32;
const HISTORICAL_KANA_START: char = '\u{1b000}';
const HISTORICAL_KANA_END: char = '\u{1b16f}';
const EXTENDED_KANA_B_START: char = '\u{1aff0}';
const EXTENDED_KANA_B_END: char = '\u{1afff}';

/// Explicit Japanese script conversion, separate from casing and width flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KanaConversion {
    /// Map supported full-width Hiragana letters/iteration marks to Katakana.
    Katakana,
    /// Map supported full-width Katakana letters/iteration marks to Hiragana.
    Hiragana,
}

impl KanaConversion {
    /// Stream supported Unicode mappings under the caller's output/work limits.
    /// Halfwidth data and combining marks retain their original code points.
    /// Unqualified source digraphs, phonetic extensions and historical kana fail.
    pub fn write(self, text: &str, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.step()?;
        let mut buffer = [0; char::MAX.len_utf8()];
        for character in text.chars() {
            output.step()?;
            if (HISTORICAL_KANA_START..=HISTORICAL_KANA_END).contains(&character)
                || (EXTENDED_KANA_B_START..=EXTENDED_KANA_B_END).contains(&character)
            {
                return Err(EvalError::UnsupportedExpression);
            }
            let mapped = match (self, character) {
                (Self::Katakana, 'ぁ'..='ゖ' | 'ゝ'..='ゞ') => {
                    char::from_u32(character as u32 + SCRIPT_OFFSET).ok_or(EvalError::Domain)?
                }
                (Self::Hiragana, 'ァ'..='ヶ' | 'ヽ'..='ヾ') => {
                    char::from_u32(character as u32 - SCRIPT_OFFSET).ok_or(EvalError::Domain)?
                }
                (Self::Hiragana, 'ヷ' | 'ヸ' | 'ヹ' | 'ヺ') => {
                    let expansion = match character {
                        'ヷ' => "わ\u{3099}",
                        'ヸ' => "ゐ\u{3099}",
                        'ヹ' => "ゑ\u{3099}",
                        _ => "を\u{3099}",
                    };
                    output.append(expansion)?;
                    continue;
                }
                (Self::Katakana, 'ゟ') | (Self::Hiragana, 'ヿ' | 'ㇰ'..='ㇿ') => {
                    return Err(EvalError::UnsupportedExpression);
                }
                _ => character,
            };
            output.append(mapped.encode_utf8(&mut buffer))?;
        }
        Ok(())
    }
}
