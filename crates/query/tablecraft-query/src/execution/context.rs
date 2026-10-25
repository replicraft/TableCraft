use super::multivalue::ParentCollections;
use super::{ExecutionOptions, QueryError, Source, aggregate::binary::Scratch as NumericScratch};
use crate::execution::cell::CellValue;
use crate::{discovery::DiscoveryState, plan::Expr};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_expressions::evaluation::{EvalLimits, Evaluator, NoBindings};
use tablecraft_expressions::numeric::OperandDeclaration;

#[derive(Clone, Copy)]
pub(super) struct ParameterValue<'a> {
    pub value: &'a Value,
    pub declaration: OperandDeclaration,
}

pub(crate) struct Runtime<'a, 'source> {
    pub(super) numeric_scratch: Vec<NumericScratch>,
    pub(super) repeatable_dynamic: bool,
    pub(super) domain_depth: usize,
    pub(super) owner: crate::external_sources::SourceNamespace,
    pub(super) source: &'source dyn Source,
    pub(super) external_sources: Vec<super::external::RuntimeSource<'source>>,
    pub(super) discovery: &'a DiscoveryState<'source>,
    pub(super) parameters: Vec<ParameterValue<'a>>,
    pub(super) options: ExecutionOptions<'a>,
    pub(super) budget: &'a mut ResourceBudget,
}
#[derive(Clone, Copy)]
pub(crate) struct Environment<'a> {
    pub(super) row: &'a [CellValue],
    pub(super) outer: &'a [OuterScope<'a>],
    pub(super) output: &'a [CellValue],
    pub(super) group: Option<&'a [Vec<CellValue>]>,
    pub(super) group_keys: Option<GroupKeys<'a>>,
    pub(super) collections: Option<&'a ParentCollections<'a>>,
}
#[derive(Clone, Copy)]
pub(super) struct OuterScope<'a> {
    pub row: &'a [CellValue],
    pub group_keys: Option<GroupKeys<'a>>,
}
#[derive(Clone, Copy)]
pub(super) struct GroupKeys<'a> {
    pub expressions: &'a [Expr],
    pub values: &'a [CellValue],
}
impl Runtime<'_, '_> {
    pub(super) fn allocate_expression_row(
        &mut self,
        width: usize,
    ) -> Result<Vec<CellValue>, QueryError> {
        self.tick()?;
        self.budget.charge(Resource::Rows, 1)?;
        self.budget
            .charge_allocation(width as u64, size_of::<CellValue>() as u64)?;
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
    pub(super) fn repeatable_key(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
        let previous = self.repeatable_dynamic;
        self.repeatable_dynamic = true;
        let result = self.scalar_key(expression, env);
        self.repeatable_dynamic = previous;
        result
    }

    pub(super) fn scalar_key(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
        let cell = self.expression_operand(expression, env)?;
        super::values::scalar(&cell)?;
        Ok(cell)
    }

    pub(super) fn evaluator(&mut self) -> Evaluator<'_> {
        let limits = EvalLimits {
            max_text_bytes: self.options.text_limit(),
            ..Default::default()
        };
        let mut evaluator = Evaluator::new(&NoBindings, self.budget, limits);
        if let Some(messages) = self.options.error_messages {
            evaluator = evaluator.with_error_messages(messages);
        }
        if let Some(session) = self.options.access_session {
            evaluator = evaluator.with_access_session(session);
        }
        if self.repeatable_dynamic {
            evaluator = evaluator.with_repeatable_dynamic_expressions();
        }
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
