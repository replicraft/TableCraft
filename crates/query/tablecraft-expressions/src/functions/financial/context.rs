//! Internal lazy argument/conversion bridge; numeric kernels retain a host-free default.
use crate::{EvalError, functions::conversions::Conversion};
use std::borrow::Cow;
use tablecraft_core::values::Value;

pub(crate) trait FinancialContext<'a> {
    fn argument(&mut self, index: usize) -> Result<Cow<'a, Value>, EvalError>;
    fn convert(&mut self, target: Conversion, value: &Value) -> Result<Value, EvalError>;
    fn step(&mut self) -> Result<(), EvalError>;
}

pub(super) struct NumericContext<'a, 'check, F> {
    pub(super) values: &'a [Value],
    pub(super) check: &'check mut F,
}

impl<'a, F: FnMut() -> Result<(), EvalError>> FinancialContext<'a> for NumericContext<'a, '_, F> {
    fn argument(&mut self, index: usize) -> Result<Cow<'a, Value>, EvalError> {
        Ok(Cow::Borrowed(&self.values[index]))
    }

    fn convert(&mut self, target: Conversion, value: &Value) -> Result<Value, EvalError> {
        target.apply(value)
    }

    fn step(&mut self) -> Result<(), EvalError> {
        (self.check)()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::functions::financial::FinancialFunction;
    use tablecraft_core::values::Double;

    #[derive(Debug, PartialEq, Eq)]
    enum Event {
        Argument(usize),
        Conversion(Conversion),
    }

    struct Recorded<'a> {
        arguments: &'a [Value],
        events: Vec<Event>,
        steps: usize,
    }

    impl<'a> FinancialContext<'a> for Recorded<'a> {
        fn argument(&mut self, index: usize) -> Result<Cow<'a, Value>, EvalError> {
            self.events.push(Event::Argument(index));
            Ok(Cow::Borrowed(&self.arguments[index]))
        }

        fn convert(&mut self, target: Conversion, value: &Value) -> Result<Value, EvalError> {
            self.events.push(Event::Conversion(target));
            target.apply(value)
        }

        fn step(&mut self) -> Result<(), EvalError> {
            self.steps += 1;
            Ok(())
        }
    }

    #[test]
    fn financial_kernels_admit_and_convert_reached_arguments_once_in_order() {
        for (function, numbers) in [
            (FinancialFunction::Pmt, vec![0.0, 2.0, 100.0, 0.0, 0.0]),
            (FinancialFunction::PV, vec![0.0, 2.0, -50.0, 0.0, 0.0]),
            (FinancialFunction::FV, vec![0.0, 2.0, -50.0, 0.0, 0.0]),
            (FinancialFunction::NPer, vec![0.0, -50.0, 100.0, 0.0, 0.0]),
            (
                FinancialFunction::IPmt,
                vec![0.0, 1.0, 2.0, 100.0, 0.0, 0.0],
            ),
            (
                FinancialFunction::PPmt,
                vec![0.0, 1.0, 2.0, 100.0, 0.0, 0.0],
            ),
            (FinancialFunction::SLN, vec![100.0, 0.0, 10.0]),
            (FinancialFunction::SYD, vec![100.0, 0.0, 4.0, 1.0]),
            (FinancialFunction::DDB, vec![1000.0, 0.0, 5.0, 1.0, 2.0]),
            (
                FinancialFunction::Rate,
                vec![1.0, -110.0, 100.0, 0.0, 0.0, 0.1],
            ),
        ] {
            let arguments: Vec<_> = numbers
                .into_iter()
                .map(|number| Value::Double(Double::from_float(number)))
                .collect();
            let mut context = Recorded {
                arguments: &arguments,
                events: Vec::new(),
                steps: 0,
            };
            function
                .apply_with_context(arguments.len(), &mut context)
                .unwrap();
            let expected: Vec<_> = (0..arguments.len())
                .flat_map(|index| {
                    [
                        Event::Argument(index),
                        Event::Conversion(Conversion::Double),
                    ]
                })
                .collect();
            assert_eq!(context.events, expected, "{function:?}");
        }

        let arguments = [
            Value::Int16(1),
            Value::Text("EUR".into()),
            Value::Text("EUR".into()),
            Value::Boolean(true),
            Value::Int16(3),
        ];
        let mut context = Recorded {
            arguments: &arguments,
            events: Vec::new(),
            steps: 0,
        };
        assert_eq!(
            FinancialFunction::EuroConvert.apply_with_context(arguments.len(), &mut context),
            Ok(Value::Double(Double::from_float(1.0)))
        );
        assert_eq!(
            context.events,
            [
                Event::Argument(0),
                Event::Conversion(Conversion::Double),
                Event::Argument(1),
                Event::Argument(2),
                Event::Argument(3),
                Event::Conversion(Conversion::Boolean),
                Event::Argument(4),
                Event::Conversion(Conversion::Integer),
            ]
        );
        // Two complete twelve-code lookups plus entry and identity publication.
        assert_eq!(context.steps, 26);
    }
}
