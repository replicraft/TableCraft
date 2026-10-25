use super::super::casing::Casing;
#[cfg(feature = "sql")]
use super::{KanaConversion, WidthConversion};
use crate::EvalError;

const CASE_MASK: i16 = 3;
const WIDE: i16 = 4;
const NARROW: i16 = 8;
const KATAKANA: i16 = 16;
const HIRAGANA: i16 = 32;
const TO_UNICODE: i16 = 64;
const FROM_UNICODE: i16 = 128;
#[cfg(feature = "sql")]
const WIDE_HIRAGANA: i16 = WIDE | HIRAGANA;
const KNOWN_FLAGS: i16 =
    CASE_MASK | WIDE | NARROW | KATAKANA | HIRAGANA | TO_UNICODE | FROM_UNICODE;

/// Case transformation applied after decoding and before encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseConversion {
    /// Preserve the original casing.
    Unchanged,
    /// Whole-string uppercase under an explicit language.
    Upper,
    /// Whole-string lowercase under an explicit language.
    Lower,
    /// Titlecase documented separator-delimited segments under an explicit language.
    Proper,
}

/// String representation transformation, independent of storage Binary carriers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodingConversion {
    /// Retain the input string representation.
    Unchanged,
    /// Decode the selected string page's bytes to Unicode.
    ToUnicode,
    /// Encode Unicode to the selected string page's bytes.
    FromUnicode,
}

/// Validated supported conversion combination; flags remain private.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Conversion {
    case: CaseConversion,
    encoding: EncodingConversion,
}

impl Conversion {
    /// Validate a checked Integer flag value. Contradictions/unknown bits fail;
    /// East Asian width/kana transformations are explicitly unsupported.
    pub fn from_code(code: i16) -> Result<Self, EvalError> {
        if code < 0 || code & !KNOWN_FLAGS != 0 {
            return Err(EvalError::Domain);
        }

        for pair in [
            WIDE | NARROW,
            KATAKANA | HIRAGANA,
            TO_UNICODE | FROM_UNICODE,
        ] {
            if code & pair == pair {
                return Err(EvalError::Domain);
            }
        }
        if code & (WIDE | NARROW | KATAKANA | HIRAGANA) != 0 {
            return Err(EvalError::UnsupportedExpression);
        }

        let case = match code & CASE_MASK {
            0 => CaseConversion::Unchanged,
            1 => CaseConversion::Upper,
            2 => CaseConversion::Lower,
            _ => CaseConversion::Proper,
        };
        let encoding = if code & TO_UNICODE != 0 {
            EncodingConversion::ToUnicode
        } else if code & FROM_UNICODE != 0 {
            EncodingConversion::FromUnicode
        } else {
            EncodingConversion::Unchanged
        };
        Ok(Self { case, encoding })
    }

    /// Selected case operation.
    pub const fn case(self) -> CaseConversion {
        self.case
    }

    /// Borrow the selected casing recipe, leaving representation conversion to
    /// the caller. The recipe requires an explicit language and metered sink.
    pub fn casing(self, text: &str) -> Option<Casing<'_>> {
        match self.case {
            CaseConversion::Unchanged => None,
            CaseConversion::Upper => Some(Casing::new(text, true)),
            CaseConversion::Lower => Some(Casing::new(text, false)),
            CaseConversion::Proper => Some(Casing::proper(text)),
        }
    }

    /// Selected string representation operation.
    pub const fn encoding(self) -> EncodingConversion {
        self.encoding
    }
}

/// Complete evaluator operation; legacy public component recipes stay unchanged.
#[cfg(feature = "sql")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    Standard(Conversion),
    Kana(KanaConversion),
    Width(WidthConversion),
    WideHiragana,
}

#[cfg(feature = "sql")]
impl Operation {
    pub(crate) fn from_code(code: i16) -> Result<Self, EvalError> {
        match code {
            WIDE => Ok(Self::Width(WidthConversion::Wide)),
            NARROW => Ok(Self::Width(WidthConversion::Narrow)),
            KATAKANA => Ok(Self::Kana(KanaConversion::Katakana)),
            HIRAGANA => Ok(Self::Kana(KanaConversion::Hiragana)),
            WIDE_HIRAGANA => Ok(Self::WideHiragana),
            _ => Conversion::from_code(code).map(Self::Standard),
        }
    }
}
