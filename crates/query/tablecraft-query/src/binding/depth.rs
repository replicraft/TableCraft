//! Validated structural depth, independent of consumable work and named cycles.
use crate::QueryError;
use std::fmt;
use tablecraft_expressions::evaluation::EvalLimits;

const _: () = {
    assert!(BindingDepth::MAX <= EvalLimits::MAX_DEPTH);
    assert!(BindingDepth::MAX <= u8::MAX as usize);
    assert!(BindingDepth::DEFAULT > 0 && BindingDepth::DEFAULT <= BindingDepth::MAX);
};

/// Caller-selected binding and prepared-plan walk depth.
/// This does not increase node, allocation, source or named-query nesting limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BindingDepth(u8);

impl BindingDepth {
    /// Existing default structural binding depth.
    pub const DEFAULT: usize = 16;
    /// Maximum explicit depth, within the scalar evaluator's depth envelope.
    pub const MAX: usize = 32;

    /// Validate a nonzero depth within the supported stack/visitor envelope.
    pub fn new(depth: usize) -> Result<Self, QueryError> {
        if !(1..=Self::MAX).contains(&depth) {
            return Err(QueryError::StructureLimit);
        }
        Ok(Self(depth as u8))
    }

    /// Selected structural depth; never zero or above the supported maximum.
    pub const fn get(self) -> usize {
        self.0 as usize
    }
}

impl Default for BindingDepth {
    fn default() -> Self {
        Self(Self::DEFAULT as u8)
    }
}

impl fmt::Display for BindingDepth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(formatter)
    }
}
