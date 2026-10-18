use super::Bindings;
use crate::{EvalError, comparison::Collation};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_sql::select::Expression;

/// Per-evaluation hard ceilings; larger supplied values are clamped to defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvalLimits {
    /// Expression-node nesting, including the root (maximum 32).
    pub max_depth: usize,
    /// Bytes in any individual materialized Text value (maximum 65,536).
    pub max_text_bytes: usize,
}
impl EvalLimits {
    /// Maximum expression-node nesting including the root.
    pub const MAX_DEPTH: usize = 32;
    /// Maximum bytes in one materialized Text value.
    pub const MAX_TEXT_BYTES: usize = 65_536;
}
impl Default for EvalLimits {
    fn default() -> Self {
        Self {
            max_depth: Self::MAX_DEPTH,
            max_text_bytes: Self::MAX_TEXT_BYTES,
        }
    }
}
/// Bounded scalar evaluator. It neither binds a schema nor executes a query.
///
/// One step is charged per visited node; Text allocation bytes are charged
/// cumulatively before allocation, including literals, bindings and intermediates.
/// Host bindings/collations own the limits and side effects of their callbacks.
pub struct Evaluator<'a> {
    pub(super) bindings: &'a dyn Bindings,
    pub(super) collation: Option<&'a dyn Collation>,
    pub(super) budget: &'a mut ResourceBudget,
    pub(super) limits: EvalLimits,
    pub(super) cancelled: Option<&'a dyn Fn() -> bool>,
}
impl<'a> Evaluator<'a> {
    /// Use explicit host bindings, a consumable operation budget and per-value limits.
    pub fn new(
        bindings: &'a dyn Bindings,
        budget: &'a mut ResourceBudget,
        limits: EvalLimits,
    ) -> Self {
        let ceiling = EvalLimits::default();
        Self {
            bindings,
            budget,
            limits: EvalLimits {
                max_depth: limits.max_depth.min(ceiling.max_depth),
                max_text_bytes: limits.max_text_bytes.min(ceiling.max_text_bytes),
            },
            collation: None,
            cancelled: None,
        }
    }
    /// Select a host collation for Text comparisons; no default is inferred.
    pub fn with_collation(mut self, collation: &'a dyn Collation) -> Self {
        self.collation = Some(collation);
        self
    }
    /// Check the host's cancellation flag before every node.
    pub fn with_cancellation(mut self, cancelled: &'a dyn Fn() -> bool) -> Self {
        self.cancelled = Some(cancelled);
        self
    }
    /// Evaluate one supported scalar tree. Both binary operands are evaluated
    /// left-to-right; Boolean truth does not suppress errors in the other operand.
    /// Counters remain consumed across calls, including failed evaluations.
    pub fn evaluate(&mut self, expression: &Expression) -> Result<Value, EvalError> {
        self.node(expression, 1)
    }
    pub(super) fn charge_text(&mut self, length: usize) -> Result<(), EvalError> {
        if length > self.limits.max_text_bytes {
            return Err(EvalError::TextLimit);
        }
        self.budget.charge(Resource::Bytes, length as u64)?;
        Ok(())
    }
    pub(super) fn text(&mut self, text: &str) -> Result<Value, EvalError> {
        self.charge_text(text.len())?;
        let mut result = String::new();
        result
            .try_reserve_exact(text.len())
            .map_err(|_| EvalError::Allocation)?;
        result.push_str(text);
        Ok(Value::Text(result))
    }
}
