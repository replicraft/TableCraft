use super::{CashFlows, EvalError, Work, finite, rate, sum::Horner};

pub(super) fn calculate(
    flows: &CashFlows,
    discount: f64,
    work: &mut Work<'_>,
) -> Result<f64, EvalError> {
    rate(discount)?;
    if flows.is_empty() {
        return Err(EvalError::Domain);
    }
    if discount == -1.0 {
        return Err(EvalError::Domain);
    }

    work.check()?;
    let factor = 1.0 / (1.0 + discount);
    if flows.scale == 0.0 {
        work.check()?;
        return Ok(0.0);
    }

    let mut sum = Horner::default();
    for value in flows.values.iter().rev() {
        work.check()?;
        sum.push(factor, flows.normalized(*value)?)?;
    }

    let result = finite((sum.value()? * factor) * flows.scale)?;
    work.check()?;
    Ok(result)
}
