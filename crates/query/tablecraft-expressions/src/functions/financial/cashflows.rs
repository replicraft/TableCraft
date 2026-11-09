//! Bounded explicit numeric cashflows, separate from stored scalar values.
mod function;
mod irr;
mod mirr;
mod npv;
#[cfg(feature = "sql")]
pub mod sql_arguments;
mod sum;
mod values;
mod work;

use crate::EvalError;
pub use function::CashFlowFunction;
pub use values::CashFlows;
pub(crate) use work::Work;
use work::{finite, number, rate};

#[cfg(test)]
mod tests;
