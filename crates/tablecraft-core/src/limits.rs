//! Explicit operation budgets. Callers charge before allocation or work.

use std::fmt;

/// Independently bounded work dimensions; these are not interchangeable units.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Resource {
    /// Bytes allocated or read, as defined by the operation.
    Bytes,
    /// Rows visited or returned, as defined by the operation.
    Rows,
    /// Work steps such as decoded pages or evaluated expression nodes.
    Steps,
}

/// A budget rejection, without source paths, queries or record contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LimitError {
    /// The exhausted dimension.
    pub resource: Resource,
    /// The budget remaining when the request was rejected.
    pub remaining: u64,
}

impl fmt::Display for LimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} resource limit exceeded ({} remaining)",
            self.resource, self.remaining
        )
    }
}

impl std::error::Error for LimitError {}

/// A consumable, per-operation budget with no implicit unlimited defaults.
///
/// Failed charges do not change any counter. A budget is not an allocator or a
/// cancellation token: operation owners must enforce it at their work boundary.
#[derive(Debug, Eq, PartialEq)]
pub struct ResourceBudget {
    bytes: u64,
    rows: u64,
    steps: u64,
}

impl ResourceBudget {
    /// Set explicit limits for all dimensions.
    pub const fn new(bytes: u64, rows: u64, steps: u64) -> Self {
        Self { bytes, rows, steps }
    }

    /// Return the remaining allowance for one resource.
    pub const fn remaining(&self, resource: Resource) -> u64 {
        match resource {
            Resource::Bytes => self.bytes,
            Resource::Rows => self.rows,
            Resource::Steps => self.steps,
        }
    }

    /// Charge a resource, rejecting exhaustion without wrapping or resetting it.
    pub fn charge(&mut self, resource: Resource, amount: u64) -> Result<(), LimitError> {
        let remaining = match resource {
            Resource::Bytes => &mut self.bytes,
            Resource::Rows => &mut self.rows,
            Resource::Steps => &mut self.steps,
        };
        *remaining = remaining.checked_sub(amount).ok_or(LimitError {
            resource,
            remaining: *remaining,
        })?;
        Ok(())
    }

    /// Check multiplication and charge bytes before a counted allocation.
    ///
    /// An unrepresentable product is exhausted capacity, never a small wrapped
    /// allocation. Successful charges return the validated byte count.
    pub fn charge_allocation(&mut self, count: u64, item_bytes: u64) -> Result<u64, LimitError> {
        let bytes = count.checked_mul(item_bytes).ok_or(LimitError {
            resource: Resource::Bytes,
            remaining: self.bytes,
        })?;
        self.charge(Resource::Bytes, bytes)?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_enforce_independent_limits_and_leave_rejected_charges_unchanged() {
        let mut budget = ResourceBudget::new(10, 3, 4);
        budget.charge(Resource::Rows, 3).unwrap();
        assert_eq!(
            budget.charge(Resource::Rows, 1),
            Err(LimitError {
                resource: Resource::Rows,
                remaining: 0
            })
        );
        assert_eq!(budget.remaining(Resource::Rows), 0);
        assert_eq!(budget.remaining(Resource::Bytes), 10);
        assert_eq!(budget.remaining(Resource::Steps), 4);
        assert_eq!(
            budget.charge_allocation(3, 4),
            Err(LimitError {
                resource: Resource::Bytes,
                remaining: 10
            })
        );
        assert_eq!(budget.remaining(Resource::Bytes), 10);
        assert_eq!(budget.charge_allocation(5, 2).unwrap(), 10);
        budget.charge(Resource::Bytes, 0).unwrap();
        budget.charge(Resource::Steps, 4).unwrap();
        assert_eq!(budget.remaining(Resource::Bytes), 0);
        assert_eq!(budget.remaining(Resource::Steps), 0);
    }

    #[test]
    fn counted_allocations_cannot_wrap_even_with_maximum_budget() {
        let mut budget = ResourceBudget::new(u64::MAX, 0, 0);
        assert_eq!(
            budget.charge_allocation(u64::MAX, 2),
            Err(LimitError {
                resource: Resource::Bytes,
                remaining: u64::MAX
            })
        );
        assert_eq!(budget.remaining(Resource::Bytes), u64::MAX);
        assert_eq!(budget.charge_allocation(u64::MAX, 0).unwrap(), 0);
        assert_eq!(budget.charge_allocation(u64::MAX, 1).unwrap(), u64::MAX);
        assert!(budget.charge_allocation(1, 1).is_err());
    }
}
