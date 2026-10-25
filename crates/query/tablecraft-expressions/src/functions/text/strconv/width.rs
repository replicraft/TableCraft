//! Japanese width mappings; unrelated compatibility characters are not normalized.
use crate::{EvalError, functions::text::output::TextOutput};
use unicode_normalization::char::{compose, decompose_canonical};

const ASCII_WIDTH_OFFSET: u32 = '！' as u32 - '!' as u32;
const HALFWIDTH_KANA_START: u32 = '｡' as u32;
// Unicode <narrow> counterparts in U+FF61..U+FF9F order.
const WIDE_KANA: &str = "。「」、・ヲァィゥェォャュョッーアイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワン\u{3099}\u{309a}";

/// Explicit Japanese width conversion, separate from script and casing flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidthConversion {
    /// Map ASCII and qualified halfwidth Japanese forms to fullwidth forms.
    Wide,
    /// Map qualified fullwidth forms to narrow characters without script conversion.
    Narrow,
}

impl WidthConversion {
    /// Write under the caller's limits. Only qualified kana pairs compose;
    /// unrelated canonical and compatibility sequences keep their representation.
    pub fn write(self, text: &str, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.step()?;
        let mut pending = None;
        let mut buffer = [0; char::MAX.len_utf8()];
        for character in text.chars() {
            output.step()?;
            if ('\u{ffa0}'..='\u{ffdc}').contains(&character)
                || ('\u{ffe0}'..='\u{ffee}').contains(&character)
            {
                return Err(EvalError::UnsupportedExpression);
            }
            match self {
                Self::Wide => {
                    let mapped = wide(character)?;
                    if let Some(previous) = pending.take() {
                        let mark = match mapped {
                            '゛' => '\u{3099}',
                            '゜' => '\u{309a}',
                            _ => mapped,
                        };
                        if is_kana(previous)
                            && matches!(mark, '\u{3099}' | '\u{309a}')
                            && let Some(composed) = compose(previous, mark)
                        {
                            pending = Some(composed);
                            continue;
                        }
                        output.append(previous.encode_utf8(&mut buffer))?;
                    }
                    pending = Some(mapped);
                }
                Self::Narrow => {
                    if let Some(mapped) = narrow(character)? {
                        output.append(mapped.encode_utf8(&mut buffer))?;
                    } else if ('ァ'..='ヺ').contains(&character) {
                        let mut decomposition = [None; 2];
                        let mut count = 0;
                        decompose_canonical(character, |part| {
                            if let Some(slot) = decomposition.get_mut(count) {
                                *slot = Some(part);
                            }
                            count += 1;
                        });
                        if count == 2 {
                            let first = narrow(decomposition[0].ok_or(EvalError::Domain)?)?;
                            let second = narrow(decomposition[1].ok_or(EvalError::Domain)?)?;
                            if let (Some(first), Some(second)) = (first, second) {
                                output.append(first.encode_utf8(&mut buffer))?;
                                output.append(second.encode_utf8(&mut buffer))?;
                                continue;
                            }
                        }
                        // Small KA/KE and WI/WE have no halfwidth counterparts.
                        return Err(EvalError::UnsupportedExpression);
                    } else {
                        output.append(character.encode_utf8(&mut buffer))?;
                    }
                }
            }
        }
        if let Some(pending) = pending {
            output.append(pending.encode_utf8(&mut buffer))?;
        }
        Ok(())
    }
}

fn is_kana(character: char) -> bool {
    ('ぁ'..='ゖ').contains(&character) || ('ァ'..='ヺ').contains(&character)
}

fn wide(character: char) -> Result<char, EvalError> {
    match character {
        ' ' => Ok('　'),
        '⦅' => Ok('｟'),
        '⦆' => Ok('｠'),
        '!'..='~' => char::from_u32(character as u32 + ASCII_WIDTH_OFFSET).ok_or(EvalError::Domain),
        '｡'..='ﾟ' => WIDE_KANA
            .chars()
            .nth((character as u32 - HALFWIDTH_KANA_START) as usize)
            .ok_or(EvalError::Domain),
        _ => Ok(character),
    }
}

fn narrow(character: char) -> Result<Option<char>, EvalError> {
    match character {
        '　' => Ok(Some(' ')),
        '｟' => Ok(Some('⦅')),
        '｠' => Ok(Some('⦆')),
        '！'..='～' => char::from_u32(character as u32 - ASCII_WIDTH_OFFSET)
            .map(Some)
            .ok_or(EvalError::Domain),
        _ => WIDE_KANA
            .chars()
            .position(|wide| wide == character)
            .map(|position| {
                char::from_u32(HALFWIDTH_KANA_START + position as u32).ok_or(EvalError::Domain)
            })
            .transpose(),
    }
}
