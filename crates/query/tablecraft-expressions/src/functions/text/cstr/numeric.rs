//! Adapt Str's bounded numeric stream without a second numeric formatting engine.
use crate::{
    EvalError,
    functions::{conversions::text::NumberFormat, text::output::TextOutput},
};

pub(in crate::functions::text) struct NumericOutput<'a, O> {
    output: &'a mut O,
    first: bool,
    separator: char,
}
impl<'a, O> NumericOutput<'a, O> {
    pub fn new(output: &'a mut O, format: NumberFormat) -> Self {
        Self {
            output,
            first: true,
            separator: format.decimal_separator(),
        }
    }
}
impl<O: TextOutput> TextOutput for NumericOutput<'_, O> {
    fn step(&mut self) -> Result<(), EvalError> {
        self.output.step()
    }
    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        self.output.step()?;
        let text = if self.first {
            self.first = false;
            text.strip_prefix(' ').unwrap_or(text)
        } else {
            text
        };
        if text.is_empty() {
            return Ok(());
        }
        if self.separator == '.' {
            return self.output.append(text);
        }
        let mut encoded = [0; char::MAX.len_utf8()];
        let separator = self.separator.encode_utf8(&mut encoded);
        for fragment in text.split_inclusive('.') {
            if let Some(prefix) = fragment.strip_suffix('.') {
                if !prefix.is_empty() {
                    self.output.append(prefix)?;
                }
                self.output.append(separator)?;
            } else {
                self.output.append(fragment)?;
            }
        }
        Ok(())
    }
}
