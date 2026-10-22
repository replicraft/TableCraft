use crate::logic::Truth;

/// Constant-space eager IN-list accumulator; no value payload is retained.
/// The host evaluates every candidate and supplies SQL equality, work limits and
/// cancellation. A prior match never suppresses later comparison errors.
pub struct Membership {
    truth: Truth,
}
impl Membership {
    /// Start with FALSE, including the empty-subquery case.
    pub const fn new() -> Self {
        Self {
            truth: Truth::False,
        }
    }
    /// Compare one candidate and accumulate SQL truth, including NULL/unknown.
    pub fn consider<E, T: ?Sized>(
        &mut self,
        value: &T,
        candidate: &T,
        mut equal: impl FnMut(&T, &T) -> Result<Truth, E>,
    ) -> Result<(), E> {
        self.truth = self.truth.or(equal(value, candidate)?);
        Ok(())
    }
    /// Finish IN or NOT IN, preserving unknown under negation.
    pub const fn finish(self, negated: bool) -> Truth {
        if negated {
            self.truth.negate()
        } else {
            self.truth
        }
    }
}
impl Default for Membership {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EvalError,
        comparison::{Comparison, compare},
    };
    use tablecraft_core::values::Value;
    #[test]
    fn null_candidates_retain_unknown_until_an_exact_match() {
        let value = Value::Int64(9_007_199_254_740_993);
        let mut membership = Membership::new();
        membership
            .consider(&value, &Value::Null, |a, b| {
                compare(Comparison::Equal, a, b, None)
            })
            .unwrap();
        assert_eq!(membership.finish(true), Truth::Unknown);
        let mut membership = Membership::new();
        for candidate in [Value::Null, value.clone()] {
            membership
                .consider(&value, &candidate, |a, b| {
                    compare(Comparison::Equal, a, b, None)
                })
                .unwrap();
        }
        assert_eq!(membership.finish(false), Truth::True);
        assert_eq!(Membership::new().finish(true), Truth::True);
    }
    #[test]
    fn earlier_true_does_not_swallow_later_errors() {
        let mut membership = Membership::new();
        membership
            .consider(&Value::Int64(1), &Value::Int64(1), |_, _| {
                Ok::<_, EvalError>(Truth::True)
            })
            .unwrap();
        assert_eq!(
            membership.consider(&Value::Int64(1), &Value::Int64(2), |_, _| Err(
                EvalError::Cancelled
            )),
            Err(EvalError::Cancelled)
        );
    }
}
