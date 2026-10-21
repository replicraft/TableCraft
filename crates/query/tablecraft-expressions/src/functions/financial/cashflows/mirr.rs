use super::{CashFlows, EvalError, Work, finite, rate};

#[derive(Default)]
struct LogSum {
    maximum: Option<f64>,
    sum: f64,
}

impl LogSum {
    fn push(&mut self, value: f64) {
        match self.maximum {
            None => {
                self.maximum = Some(value);
                self.sum = 1.;
            }
            Some(old) if value > old => {
                self.sum = self.sum * (old - value).exp() + 1.;
                self.maximum = Some(value);
            }
            Some(old) => self.sum += (value - old).exp(),
        }
    }

    fn value(&self) -> Result<f64, EvalError> {
        self.maximum
            .map(|v| v + self.sum.ln())
            .ok_or(EvalError::Domain)
    }
}

pub(super) fn calculate(
    flows: &CashFlows,
    finance: f64,
    reinvest: f64,
    work: &mut Work<'_>,
) -> Result<f64, EvalError> {
    rate(finance)?;
    rate(reinvest)?;
    flows.mixed()?;
    if finance <= -1.0 || reinvest <= -1.0 {
        return Err(EvalError::Domain);
    }

    let finance = finance.ln_1p();
    let reinvest = reinvest.ln_1p();
    let end = (flows.len() - 1) as f64;
    let mut positive = LogSum::default();
    let mut negative = LogSum::default();

    for (index, value) in flows.values.iter().enumerate() {
        work.check()?;
        let time = index as f64;
        if *value > 0.0 {
            positive.push(value.ln() + (end - time) * reinvest);
        } else if *value < 0.0 {
            negative.push((-value).ln() - time * finance);
        }
    }

    let result = finite(((positive.value()? - negative.value()?) / end).exp_m1())?;
    work.check()?;
    Ok(result)
}
