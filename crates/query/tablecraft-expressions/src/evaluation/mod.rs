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
mod arrays;
mod bindings;
mod byte_output;
mod byte_search;
mod byte_strings;
mod cashflows;
mod clock;
mod conditional;
mod context;
mod conversions;
mod date_conversions;
mod dynamic;
mod error_messages;
#[cfg(test)]
mod euro_tests;
mod financial;
mod functions;
mod guid;
mod inspection;
mod operators;
mod random;
mod session;
mod strconv;
mod text_functions;
mod text_output;
mod value_inspection;
#[cfg(test)]
mod value_inspection_tests;
#[cfg(test)]
mod variant_tests;
mod variants;
mod walk;
mod weekday;
pub use bindings::{Bindings, NoBindings};
pub use context::{EvalLimits, Evaluator};

#[cfg(test)]
mod cashflow_tests;
#[cfg(test)]
mod function_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod text_function_tests;

#[cfg(test)]
mod format_tests;
