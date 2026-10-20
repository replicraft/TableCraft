//! Evaluate one owned syntax expression over values supplied by a host.
//!
//! ```
//! use tablecraft_core::{limits::ResourceBudget, values::Value};
//! use tablecraft_expressions::evaluation::{EvalLimits, Evaluator, NoBindings};
//! use tablecraft_sql::select::{BinaryOperator, Expression};
//! let expression = Expression::Binary {
//!     left: Box::new(Expression::Integer(2)),
//!     operator: BinaryOperator::Add,
//!     right: Box::new(Expression::Integer(3)),
//! };
//! let mut budget = ResourceBudget::new(0, 0, 3);
//! let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
//! assert_eq!(evaluator.evaluate(&expression)?, Value::Int64(5));
//! # Ok::<(), tablecraft_expressions::EvalError>(())
//! ```
mod bindings;
mod context;
mod conversions;
mod functions;
mod operators;
mod text_functions;
mod text_output;
mod walk;
pub use bindings::{Bindings, NoBindings};
pub use context::{EvalLimits, Evaluator};

#[cfg(test)]
mod function_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod text_function_tests;
