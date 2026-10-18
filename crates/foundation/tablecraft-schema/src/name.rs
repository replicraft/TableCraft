use crate::SchemaError;

/// Validated nonempty, control-free schema identity; size limits are profile-owned.
/// Equality preserves spelling; NameComparison defines lookup/duplicate semantics.
/// No Debug/Serde implementation exposes or bypasses the validated name.
#[derive(Clone, PartialEq, Eq)]
pub struct SchemaName(String);

impl SchemaName {
    /// Validate the shared name invariants without imposing a backend length limit.
    pub fn new(name: String) -> Result<Self, SchemaError> {
        if name.is_empty() || name.chars().any(char::is_control) {
            return Err(SchemaError::Name);
        }
        Ok(Self(name))
    }
    /// Original spelling, without implicit case folding or normalization.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Explicit identifier comparison; neither option claims Access locale fidelity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameComparison {
    /// Exact Unicode spelling.
    Exact,
    /// Fold ASCII case only; other characters require exact spelling.
    AsciiInsensitive,
}
impl NameComparison {
    /// Compare identities under the caller's selected policy.
    pub fn equals(self, left: &str, right: &str) -> bool {
        match self {
            Self::Exact => left == right,
            Self::AsciiInsensitive => left.eq_ignore_ascii_case(right),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_invariants_do_not_impose_query_profile_limits() {
        for name in ["", "bad\nname", "bad\0name"] {
            assert!(SchemaName::new(name.into()).is_err());
        }
        assert_eq!(
            SchemaName::new("x".repeat(300)).unwrap().as_str().len(),
            300
        );
        assert!(!NameComparison::Exact.equals("ID", "id"));
        assert!(NameComparison::AsciiInsensitive.equals("ID", "id"));
        assert!(!NameComparison::AsciiInsensitive.equals("É", "é"));
    }
}
