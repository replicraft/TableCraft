//! Independent streaming states reuse the buffered numeric aggregate kernels.
use super::{Environment, Runtime, average::Mean, sum::Sum};
use crate::{
    QueryError,
    plan::{Aggregate, Expr},
};
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;

pub(in crate::execution) struct Accumulator {
    state: State,
}

enum State {
    Count(i64),
    Sum { count: i64, state: Sum },
    Average { count: i64, state: Mean },
}

impl Accumulator {
    pub(in crate::execution) fn new(kind: Aggregate) -> Result<Self, QueryError> {
        match kind {
            Aggregate::Count => Ok(Self {
                state: State::Count(0),
            }),
            Aggregate::Sum => Ok(Self {
                state: State::Sum {
                    count: 0,
                    state: Sum::default(),
                },
            }),
            Aggregate::Average => Ok(Self {
                state: State::Average {
                    count: 0,
                    state: Mean::default(),
                },
            }),
            _ => Err(QueryError::Unsupported),
        }
    }
}

impl Runtime<'_, '_> {
    pub(in crate::execution) fn push_progressive_aggregate(
        &mut self,
        accumulator: &mut Accumulator,
        argument: Option<&Expr>,
        environment: Environment<'_>,
    ) -> Result<(), QueryError> {
        if let State::Count(count) = &mut accumulator.state {
            if let Some(argument) = argument
                && !self.count_argument(argument, environment)?
            {
                return Ok(());
            }
            *count = count.checked_add(1).ok_or(EvalError::Overflow)?;
            return Ok(());
        }

        let value = self.numeric_aggregate_argument(argument, environment)?;
        if matches!(value, Value::Null) {
            return Ok(());
        }
        // Preserve the buffered non-NULL prefix counter and its overflow check
        // before handing the unchanged carrier to the owning numeric kernel.
        let count = match &mut accumulator.state {
            State::Sum { count, .. } | State::Average { count, .. } => count,
            State::Count(_) => unreachable!("handled before numeric publication"),
        };
        *count = count.checked_add(1).ok_or(EvalError::Overflow)?;

        match &mut accumulator.state {
            State::Sum { state, .. } => state.push(
                &value,
                self.budget,
                self.options.cancelled,
                &mut self.numeric_scratch,
            ),
            State::Average { state, .. } => state.push(
                &value,
                self.budget,
                self.options.cancelled,
                &mut self.numeric_scratch,
            ),
            State::Count(_) => unreachable!("handled before numeric publication"),
        }
    }

    pub(in crate::execution) fn finish_progressive_aggregate(
        &mut self,
        accumulator: Accumulator,
    ) -> Result<Value, QueryError> {
        match accumulator.state {
            State::Count(count) => Ok(Value::Int64(count)),
            State::Sum { state, .. } => state.finish(
                self.budget,
                self.options.cancelled,
                &mut self.numeric_scratch,
            ),
            State::Average { count, state } => state.finish(
                count,
                self.budget,
                self.options.cancelled,
                &mut self.numeric_scratch,
            ),
        }
    }
}
