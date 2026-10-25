//! Compose the existing Japanese shape services after optional ICU casing.
use super::{ByteOperand, EvalError, Evaluator, KanaConversion, WidthConversion};

impl Evaluator<'_> {
    pub(super) fn strconv_japanese(
        &mut self,
        input: ByteOperand,
        width: Option<WidthConversion>,
        kana: Option<KanaConversion>,
    ) -> Result<ByteOperand, EvalError> {
        // Widen only when explicitly requested. Microsoft's mapping contract
        // leaves halfwidth Katakana unchanged under Hiragana without FULLWIDTH;
        // a requested narrow result must not silently acquire that extra flag.
        let input = if width == Some(WidthConversion::Wide) {
            self.strconv_width(input, WidthConversion::Wide)?
        } else {
            input
        };
        let input = if let Some(kana) = kana {
            self.strconv_kana(input, kana)?
        } else {
            input
        };
        if width == Some(WidthConversion::Narrow) {
            self.strconv_width(input, WidthConversion::Narrow)
        } else {
            Ok(input)
        }
    }
}
