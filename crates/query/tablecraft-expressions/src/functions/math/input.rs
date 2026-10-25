//! Lazy argument admission and conversion share one context without eager publication.
use crate::{EvalError, functions::conversions::Conversion};
use std::borrow::Cow;
use tablecraft_core::values::Value;

pub(crate) trait MathContext<'a> {
    fn argument(&mut self, index: usize) -> Result<Cow<'a, Value>, EvalError>;

    fn convert(&mut self, target: Conversion, value: &Value) -> Result<Value, EvalError>;
}

pub(super) struct ScalarArguments<'a> {
    pub(super) values: &'a [Value],
}

impl<'a> MathContext<'a> for ScalarArguments<'a> {
    fn argument(&mut self, index: usize) -> Result<Cow<'a, Value>, EvalError> {
        Ok(Cow::Borrowed(&self.values[index]))
    }

    fn convert(&mut self, target: Conversion, value: &Value) -> Result<Value, EvalError> {
        target.apply(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::functions::math::{MathFunction, random::RndState};

    #[derive(Debug, PartialEq, Eq)]
    enum Event {
        Argument(usize),
        Conversion(Conversion),
    }

    struct Recorded<'a> {
        text: &'a Value,
        events: Vec<Event>,
    }

    impl<'a> MathContext<'a> for Recorded<'a> {
        fn argument(&mut self, index: usize) -> Result<Cow<'a, Value>, EvalError> {
            self.events.push(Event::Argument(index));
            Ok(Cow::Borrowed(self.text))
        }

        fn convert(&mut self, target: Conversion, value: &Value) -> Result<Value, EvalError> {
            assert_eq!(value, self.text);
            self.events.push(Event::Conversion(target));
            target.apply(&Value::Int16(1))
        }
    }

    #[test]
    fn every_math_kernel_admits_and_converts_each_reached_argument_once() {
        for function in [
            MathFunction::Abs,
            MathFunction::Sgn,
            MathFunction::Int,
            MathFunction::Fix,
            MathFunction::Sqr,
            MathFunction::Exp,
            MathFunction::Log,
            MathFunction::Sin,
            MathFunction::Cos,
            MathFunction::Tan,
            MathFunction::Atn,
            MathFunction::Round,
            MathFunction::Rgb,
            MathFunction::QbColor,
            MathFunction::Rnd,
        ] {
            let mut context = Recorded {
                text: &Value::Text("1".into()),
                events: Vec::new(),
            };
            let (count, expected) = match function {
                MathFunction::Round => (
                    2,
                    vec![
                        Event::Argument(1),
                        Event::Conversion(Conversion::Long),
                        Event::Argument(0),
                        Event::Conversion(Conversion::Double),
                    ],
                ),
                MathFunction::Rgb => (
                    3,
                    vec![
                        Event::Argument(0),
                        Event::Conversion(Conversion::Integer),
                        Event::Argument(1),
                        Event::Conversion(Conversion::Integer),
                        Event::Argument(2),
                        Event::Conversion(Conversion::Integer),
                    ],
                ),
                MathFunction::QbColor => (
                    1,
                    vec![Event::Argument(0), Event::Conversion(Conversion::Integer)],
                ),
                MathFunction::Rnd => (
                    1,
                    vec![Event::Argument(0), Event::Conversion(Conversion::Single)],
                ),
                _ => (
                    1,
                    vec![Event::Argument(0), Event::Conversion(Conversion::Double)],
                ),
            };
            function
                .apply_with_context(count, &mut context, Some(&RndState::default()))
                .unwrap();
            assert_eq!(context.events, expected, "{function:?}");
        }
    }
}
