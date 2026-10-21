//! Exact explicit LCID identity; no OS discovery or implicit locale mapping.
use crate::EvalError;

const RESERVED_BITS: u32 = 0xfff0_0000;

/// Structurally valid Windows LCID supplied by a caller or expression.
///
/// Retains the language and alternate-sort bits. This validates only the numeric
/// structure, not whether any provider implements the locale. Default/custom
/// identities must be explicitly resolved by the host; no machine setting is read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocaleId(u32);
impl LocaleId {
    /// Reject zero and reserved high bits; retain all language/alternate-sort bits.
    pub const fn new(raw: u32) -> Result<Self, EvalError> {
        if raw == 0 || raw & RESERVED_BITS != 0 {
            Err(EvalError::Domain)
        } else {
            Ok(Self(raw))
        }
    }
    /// Exact host identity, including any alternate-sort selection.
    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_structure_retains_alternate_sorts_and_does_not_qualify_locales() {
        for raw in [1, 1033, 0x10407, 0x1000, 0xfffff] {
            assert_eq!(LocaleId::new(raw).unwrap().raw(), raw);
        }
        for raw in [0, 0x100000, 0x80000409, u32::MAX] {
            assert_eq!(LocaleId::new(raw), Err(EvalError::Domain));
        }
    }
}
