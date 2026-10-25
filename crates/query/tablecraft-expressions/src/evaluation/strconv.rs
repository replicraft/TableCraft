//! StrConv owns representation conversion without changing stored scalar types.
use super::{Evaluator, text_arguments};
use crate::{
    EvalError,
    comparison::LocaleId,
    functions::{
        conversions::Conversion as NumericConversion,
        text::{
            byte_string::{ByteOperand, ByteString, ExpressionByteOperand},
            strconv::{
                self, CaseConversion, EncodingConversion, KanaConversion, Operation,
                StringLocaleProfile, WidthConversion,
            },
        },
    },
};
use tablecraft_core::values::{ExpressionValue, Value};

const JAPANESE_LCID: u32 = 1041;

impl Evaluator<'_> {
    /// Admit eager controls before the source while preserving raw string bytes.
    pub fn apply_expression_strconv(
        &mut self,
        input: ExpressionByteOperand,
        conversion: &ExpressionValue,
        locale: Option<&ExpressionValue>,
    ) -> Result<ByteOperand, EvalError> {
        self.text_expression_limits(std::slice::from_ref(conversion))?;
        if let Some(locale) = locale {
            self.text_expression_limits(std::slice::from_ref(locale))?;
        }
        let empty_number = Value::Int32(0);
        self.strconv_inputs(
            || input.into_string(),
            locale.is_some(),
            |index| {
                let value = if index == 0 {
                    conversion
                } else {
                    locale.ok_or(EvalError::UnsupportedExpression)?
                };
                text_arguments::scalar(value, &empty_number)
            },
        )
    }

    /// Apply StrConv to written-order evaluated arguments. Supported explicit
    /// LCIDs override execution-local language/page settings for this call only.
    pub fn apply_strconv(
        &mut self,
        input: ByteOperand,
        conversion: &Value,
        locale: Option<&Value>,
    ) -> Result<ByteOperand, EvalError> {
        self.strconv_inputs(
            || Ok(input),
            locale.is_some(),
            |index| {
                if index == 0 {
                    Ok(conversion)
                } else {
                    locale.ok_or(EvalError::UnsupportedExpression)
                }
            },
        )
    }

    fn strconv_inputs<'a>(
        &mut self,
        source: impl FnOnce() -> Result<ByteOperand, EvalError>,
        has_locale: bool,
        mut argument: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    ) -> Result<ByteOperand, EvalError> {
        self.check(0)?;
        let conversion = argument(0)?;
        if matches!(conversion, Value::Null) {
            return Err(EvalError::InvalidNullConversion);
        }
        let Value::Int16(code) = self.apply_conversion(NumericConversion::Integer, conversion)?
        else {
            return Err(EvalError::TypeMismatch);
        };
        let operation = Operation::from_code(code)?;
        let locale = has_locale
            .then(|| {
                let value = argument(1)?;
                if matches!(value, Value::Null) {
                    return Err(EvalError::InvalidNullConversion);
                }
                let Value::Int32(code) = self.apply_conversion(NumericConversion::Long, value)?
                else {
                    return Err(EvalError::TypeMismatch);
                };
                let id = u32::try_from(code).map_err(|_| EvalError::Domain)?;
                let id = LocaleId::new(id)?;
                if matches!(operation, Operation::Kana(_) | Operation::Width(_)) {
                    if id.raw() != JAPANESE_LCID {
                        return Err(EvalError::UnsupportedExpression);
                    }
                    Ok(None)
                } else {
                    StringLocaleProfile::from_lcid(id).map(Some)
                }
            })
            .transpose()?
            .flatten();
        if matches!(operation, Operation::Kana(_) | Operation::Width(_)) && !has_locale {
            return Err(EvalError::UnsupportedExpression);
        }
        let input = source()?;
        if matches!(&input, ByteOperand::Scalar(Value::Null)) {
            return Ok(ByteOperand::Scalar(Value::Null));
        }

        let input = self.coerce_byte_string_source(input)?;
        let conversion = match operation {
            Operation::Kana(kana) => return self.strconv_kana(input, kana),
            Operation::Width(width) => return self.strconv_width(input, width),
            Operation::Standard(conversion) => conversion,
        };
        let page = locale
            .as_ref()
            .map(StringLocaleProfile::code_page)
            .or_else(|| self.character_code_page.map(Into::into));
        let language = locale
            .as_ref()
            .map(StringLocaleProfile::case_locale)
            .or(self.case_locale);

        let input = if conversion.encoding() == EncodingConversion::ToUnicode {
            let bytes = match input {
                ByteOperand::Bytes(bytes) => bytes,
                ByteOperand::Scalar(Value::Text(text)) => self.encode_byte_string(&text)?,
                ByteOperand::Scalar(Value::Binary(bytes)) => ByteString::from_owned(bytes)?,
                ByteOperand::Scalar(_) => return Err(EvalError::TypeMismatch),
            };
            let mut output = super::text_output::Output {
                budget: self.budget,
                cancelled: self.cancelled,
                max_bytes: self.limits.max_text_bytes,
                text: String::new(),
            };
            match page {
                Some(page) => page.decode(bytes.as_bytes(), &mut output)?,
                None => strconv::decode_ansi(bytes.as_bytes(), None, &mut output)?,
            }
            ByteOperand::Scalar(Value::Text(output.text))
        } else {
            if matches!(&input, ByteOperand::Scalar(value) if !matches!(value, Value::Text(_))) {
                return Err(EvalError::TypeMismatch);
            }
            input
        };
        if conversion.case() == CaseConversion::Unchanged
            && conversion.encoding() == EncodingConversion::Unchanged
        {
            if matches!(&input, ByteOperand::Scalar(Value::Text(text)) if text.len() > self.limits.max_text_bytes)
            {
                return Err(EvalError::TextLimit);
            }
            return Ok(input);
        }
        let Value::Text(mut text) = self.publish_byte_operand(input)? else {
            return Err(EvalError::TypeMismatch);
        };
        if text.len() > self.limits.max_text_bytes {
            return Err(EvalError::TextLimit);
        }
        if let Some(recipe) = conversion.casing(&text) {
            let mut output = super::text_output::Output {
                budget: self.budget,
                cancelled: self.cancelled,
                max_bytes: self.limits.max_text_bytes,
                text: String::new(),
            };
            recipe.write(language, &mut output)?;
            text = output.text;
        }
        if conversion.encoding() == EncodingConversion::FromUnicode {
            let mut output = super::byte_output::Output {
                budget: self.budget,
                cancelled: self.cancelled,
                max_bytes: self.limits.max_text_bytes,
                bytes: Vec::new(),
            };
            match page {
                Some(page) => page.encode(&text, &mut output)?,
                None => strconv::encode_ansi(&text, None, &mut output)?,
            }
            return ByteString::from_owned(output.bytes).map(ByteOperand::Bytes);
        }
        Ok(ByteOperand::Scalar(Value::Text(text)))
    }

    fn strconv_width(
        &mut self,
        input: ByteOperand,
        width: WidthConversion,
    ) -> Result<ByteOperand, EvalError> {
        let Value::Text(text) = self.publish_byte_operand(input)? else {
            return Err(EvalError::TypeMismatch);
        };
        if text.len() > self.limits.max_text_bytes {
            return Err(EvalError::TextLimit);
        }
        let mut output = super::text_output::Output {
            budget: self.budget,
            cancelled: self.cancelled,
            max_bytes: self.limits.max_text_bytes,
            text: String::new(),
        };
        width.write(&text, &mut output)?;
        Ok(ByteOperand::Scalar(Value::Text(output.text)))
    }

    fn strconv_kana(
        &mut self,
        input: ByteOperand,
        kana: KanaConversion,
    ) -> Result<ByteOperand, EvalError> {
        let Value::Text(text) = self.publish_byte_operand(input)? else {
            return Err(EvalError::TypeMismatch);
        };
        if text.len() > self.limits.max_text_bytes {
            return Err(EvalError::TextLimit);
        }
        let mut output = super::text_output::Output {
            budget: self.budget,
            cancelled: self.cancelled,
            max_bytes: self.limits.max_text_bytes,
            text: String::new(),
        };
        kana.write(&text, &mut output)?;
        Ok(ByteOperand::Scalar(Value::Text(output.text)))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expression_tests;

#[cfg(test)]
mod kana_tests;

#[cfg(test)]
mod width_tests;
