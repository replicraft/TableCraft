//! Immutable ordered numeric cashflows and their bounded construction.
use super::{Work, irr, mirr, npv, number};
use crate::{EvalError, arrays::ArrayValue, functions::financial::FinancialFunction};
use std::fmt;
use tablecraft_core::{limits::ResourceBudget, values::Value};

/// Immutable ordered cashflows, retaining every zero and its original position.
pub struct CashFlows {
    pub(super) values: Vec<f64>,
    pub(super) scale: f64,
    positive: bool,
    negative: bool,
}

impl fmt::Debug for CashFlows {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CashFlows")
            .field("len", &self.values.len())
            .finish_non_exhaustive()
    }
}

impl CashFlows {
    /// Maximum elements accepted before allocation or conversion.
    pub const MAX_ELEMENTS: usize = ArrayValue::MAX_ELEMENTS;

    /// Copy finite numeric/Boolean values under the caller's cumulative budget.
    /// NULL and nonnumeric values fail. Empty sequences are data; operations may
    /// reject them. Debug output never prints payloads. No source/provider is read.
    pub fn new(
        values: &[Value],
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<Self, EvalError> {
        if values.len() > Self::MAX_ELEMENTS {
            return Err(EvalError::ArrayLimit);
        }

        let mut work = Work { budget, cancelled };
        work.check()?;
        work.budget
            .charge_allocation(values.len() as u64, size_of::<f64>() as u64)?;
        let mut numbers = Vec::new();
        numbers
            .try_reserve_exact(values.len())
            .map_err(|_| EvalError::Allocation)?;

        let mut scale = 0_f64;
        let mut positive = false;
        let mut negative = false;

        for value in values {
            work.check()?;
            let value = number(value)?;
            scale = scale.max(value.abs());
            positive |= value > 0.;
            negative |= value < 0.;
            numbers.push(value);
        }

        work.check()?;

        Ok(Self {
            values: numbers,
            scale,
            positive,
            negative,
        })
    }

    /// Number of positions, including zero-valued positions.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether there are no positions.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Discount positions beginning at period one. Single-sign nonempty series
    /// are allowed; zero rates sum all positions without deleting zero entries.
    pub fn npv(
        &self,
        rate: f64,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<f64, EvalError> {
        npv::calculate(self, rate, &mut Work { budget, cancelled })
    }

    /// Solve period-zero-based cashflows with the shared 20-iteration solver.
    /// Both signs are required; omission uses 0.1. Every pass is metered.
    pub fn irr(
        &self,
        guess: Option<f64>,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<f64, EvalError> {
        irr::calculate(
            self,
            guess.unwrap_or(FinancialFunction::DEFAULT_GUESS),
            &mut Work { budget, cancelled },
        )
    }

    /// Combine financed negative cashflows and reinvested positive cashflows,
    /// with period zero at the first position and the original length preserved.
    pub fn mirr(
        &self,
        finance: f64,
        reinvest: f64,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<f64, EvalError> {
        mirr::calculate(self, finance, reinvest, &mut Work { budget, cancelled })
    }

    pub(super) fn normalized(&self, value: f64) -> Result<f64, EvalError> {
        let result = value / self.scale;

        if value != 0.0 && result == 0.0 {
            Err(EvalError::PrecisionLoss)
        } else {
            Ok(result)
        }
    }

    pub(super) fn mixed(&self) -> Result<(), EvalError> {
        if self.values.len() < 2 || !self.positive || !self.negative {
            Err(EvalError::Domain)
        } else {
            Ok(())
        }
    }
}
