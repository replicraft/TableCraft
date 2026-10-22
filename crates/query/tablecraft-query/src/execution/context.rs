use super::{ExecutionOptions, QueryError, Source};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{ExpressionValue, Value},
};
use tablecraft_expressions::evaluation::{EvalLimits, Evaluator, NoBindings};

pub(crate) struct Runtime<'a> {
    pub(super) domain_depth: usize,
    pub(super) source: &'a dyn Source,
    pub(super) parameters: Vec<&'a Value>,
    pub(super) options: ExecutionOptions<'a>,
    pub(super) budget: &'a mut ResourceBudget,
}
#[derive(Clone, Copy)]
pub(crate) struct Environment<'a> {
    pub(super) row: &'a [ExpressionValue],
    pub(super) outer: &'a [&'a [ExpressionValue]],
    pub(super) output: &'a [ExpressionValue],
    pub(super) group: Option<&'a [Vec<ExpressionValue>]>,
}
impl Runtime<'_> {
    pub(super) fn allocate_expression_row(
        &mut self,
        width: usize,
    ) -> Result<Vec<ExpressionValue>, QueryError> {
        self.tick()?;
        self.budget.charge(Resource::Rows, 1)?;
        self.budget
            .charge_allocation(width as u64, size_of::<ExpressionValue>() as u64)?;
        let mut row = Vec::new();
        row.try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        Ok(row)
    }

    pub(super) fn tick(&mut self) -> Result<(), QueryError> {
        if self.options.cancelled.is_some_and(|f| f()) {
            return Err(QueryError::Cancelled);
        }
        self.budget.charge(Resource::Steps, 1)?;
        Ok(())
    }
    pub(super) fn evaluator(&mut self) -> Evaluator<'_> {
        let mut evaluator = Evaluator::new(&NoBindings, self.budget, EvalLimits::default());
        if let Some(locale) = self.options.case_locale {
            evaluator = evaluator.with_case_locale(locale);
        }
        if let Some(random) = self.options.random {
            evaluator = evaluator.with_random(random);
        }
        if let Some(clock) = self.options.clock {
            evaluator = evaluator.with_clock(clock);
        }
        if let Some(rule) = self.options.first_week_of_year {
            evaluator = evaluator.with_first_week_of_year(rule);
        }
        if let Some(day) = self.options.first_day_of_week {
            evaluator = evaluator.with_first_day_of_week(day);
        }
        if let Some(year) = self.options.reference_year {
            evaluator = evaluator.with_reference_year(year);
        }
        if let Some(format) = self.options.date_format {
            evaluator = evaluator.with_date_format(format);
        }
        if let Some(format) = self.options.number_format {
            evaluator = evaluator.with_number_format(format);
        }
        if let Some(page) = self.options.character_code_page {
            evaluator = evaluator.with_character_code_page(page);
        }
        if let Some(window) = self.options.year_window {
            evaluator = evaluator.with_year_window(window);
        }
        if let Some(mode) = self.options.like_mode {
            evaluator = evaluator.with_like_mode(mode);
        }
        if let Some(c) = self.options.collation {
            evaluator = evaluator.with_collation(c);
        }
        if let Some(c) = self.options.cancelled {
            evaluator = evaluator.with_cancellation(c);
        }
        evaluator
    }
    pub(super) fn allocate_row(&mut self, width: usize) -> Result<Vec<Value>, QueryError> {
        self.tick()?;
        self.budget.charge(Resource::Rows, 1)?;
        self.budget
            .charge_allocation(width as u64, std::mem::size_of::<Value>() as u64)?;
        let mut row = Vec::new();
        row.try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        Ok(row)
    }
    pub(super) fn push<T>(&mut self, vec: &mut Vec<T>, value: T) -> Result<(), QueryError> {
        self.budget
            .charge(Resource::Bytes, std::mem::size_of::<T>() as u64)?;
        vec.try_reserve(1).map_err(|_| QueryError::Allocation)?;
        vec.push(value);
        Ok(())
    }
}
