use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use tablecraft_core::{limits::ResourceBudget, values::Double};

#[test]
fn financial_numeric_text_reuses_double_and_typed_euro_option_conversion() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::DeDe);
    for function in [
        FinancialFunction::SLN,
        FinancialFunction::PV,
        FinancialFunction::FV,
        FinancialFunction::Pmt,
        FinancialFunction::NPer,
    ] {
        let numbers = if function == FinancialFunction::SLN {
            vec![Value::Int16(100), Value::Int16(0), Value::Int16(10)]
        } else {
            vec![
                Value::Double(Double::from_float(0.1)),
                Value::Int16(10),
                Value::Int16(100),
            ]
        };
        let texts = if function == FinancialFunction::SLN {
            vec![
                Value::Text("100".into()),
                Value::Text("0".into()),
                Value::Text("10".into()),
            ]
        } else {
            vec![
                Value::Text("0,1".into()),
                Value::Text("10".into()),
                Value::Text("100".into()),
            ]
        };
        assert_eq!(
            evaluator.apply_financial(function, &texts),
            function.apply(&numbers)
        );
    }
    let euro = [
        Value::Text("1,5".into()),
        Value::Text("DEM".into()),
        Value::Text("EUR".into()),
        Value::Text("1".into()),
        Value::Text("3,5".into()),
    ];
    let numeric = [
        Value::Double(Double::from_float(1.5)),
        Value::Text("DEM".into()),
        Value::Text("EUR".into()),
        Value::Boolean(true),
        Value::Int16(4),
    ];
    assert_eq!(
        evaluator.apply_financial(FinancialFunction::EuroConvert, &euro),
        FinancialFunction::EuroConvert.apply(&numeric)
    );
}

#[test]
fn contextual_financial_validation_preserves_order_and_cancellation() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_financial(
            FinancialFunction::SLN,
            &[Value::Null, Value::Text("bad".into()), Value::Int16(1)]
        ),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(
        evaluator.apply_financial(
            FinancialFunction::SLN,
            &[Value::Text("1".into()), Value::Int16(0), Value::Int16(1)]
        ),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        evaluator.apply_financial(
            FinancialFunction::EuroConvert,
            &[
                Value::Int16(1),
                Value::Text("USD".into()),
                Value::Text("EUR".into()),
                Value::Text("bad".into())
            ]
        ),
        Err(EvalError::Domain)
    );
    let mut budget = ResourceBudget::new(0, 0, 1);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&|| true)
            .apply_financial(
                FinancialFunction::SLN,
                &[Value::Int16(1), Value::Int16(0), Value::Int16(1)]
            ),
        Err(EvalError::Cancelled)
    );
}
