//! StrConv owns representation conversion without changing stored scalar types.
use super::Evaluator;
use crate::{
    EvalError,
    comparison::LocaleId,
    functions::{
        conversions::Conversion as NumericConversion,
        text::{
            byte_string::{ByteOperand, ByteString},
            strconv::{self, CaseConversion, Conversion, EncodingConversion, LocaleProfile},
        },
    },
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    /// Apply StrConv to written-order evaluated arguments. Supported explicit
    /// LCIDs override execution-local language/page settings for this call only.
    pub fn apply_strconv(
        &mut self,
        input: ByteOperand,
        conversion: &Value,
        locale: Option<&Value>,
    ) -> Result<ByteOperand, EvalError> {
        self.check(0)?;
        if matches!(conversion, Value::Null) {
            return Err(EvalError::InvalidNullConversion);
        }
        let Value::Int16(code) = self.apply_conversion(NumericConversion::Integer, conversion)?
        else {
            return Err(EvalError::TypeMismatch);
        };
        let conversion = Conversion::from_code(code)?;
        let locale = locale
            .map(|value| {
                if matches!(value, Value::Null) {
                    return Err(EvalError::InvalidNullConversion);
                }
                let Value::Int32(code) = self.apply_conversion(NumericConversion::Long, value)?
                else {
                    return Err(EvalError::TypeMismatch);
                };
                let id = u32::try_from(code).map_err(|_| EvalError::Domain)?;
                LocaleProfile::from_lcid(LocaleId::new(id)?)
            })
            .transpose()?;
        if matches!(&input, ByteOperand::Scalar(Value::Null)) {
            return Ok(ByteOperand::Scalar(Value::Null));
        }

        let input = self.coerce_byte_string_source(input)?;
        let page = locale
            .as_ref()
            .map(LocaleProfile::code_page)
            .or(self.character_code_page);
        let language = locale
            .as_ref()
            .map(LocaleProfile::case_locale)
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
            strconv::decode_ansi(bytes.as_bytes(), page, &mut output)?;
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
            strconv::encode_ansi(&text, page, &mut output)?;
            return ByteString::from_owned(output.bytes).map(ByteOperand::Bytes);
        }
        Ok(ByteOperand::Scalar(Value::Text(text)))
    }
}

#[cfg(test)]
mod tests;
