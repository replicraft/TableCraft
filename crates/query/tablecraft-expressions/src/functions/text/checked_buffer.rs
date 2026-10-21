//! Retain the origin of host failures before interpreting lexical parser failures.
use crate::{
    EvalError,
    functions::text::output::{NumericBuffer, TextOutput},
};

pub(crate) struct CheckedBuffer<'a, B> {
    pub inner: &'a mut B,
    pub failure: Option<EvalError>,
}
impl<B: NumericBuffer> TextOutput for CheckedBuffer<'_, B> {
    fn step(&mut self) -> Result<(), EvalError> {
        let result = self.inner.step();
        if let Err(error) = result {
            self.failure = Some(error);
        }
        result
    }
    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        let result = self.inner.append(text);
        if let Err(error) = result {
            self.failure = Some(error);
        }
        result
    }
}
impl<B: NumericBuffer> NumericBuffer for CheckedBuffer<'_, B> {
    fn text(&self) -> &str {
        self.inner.text()
    }
}
