//! Caller-issued semantic identity domains and source generations.

macro_rules! identity {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name(u128);

        impl $name {
            /// Wrap a caller-issued token without changing its identity domain.
            pub const fn from_u128(token: u128) -> Self {
                Self(token)
            }

            /// Return the token for an explicit mapping or protocol encoder.
            pub const fn as_u128(self) -> u128 {
                self.0
            }
        }
    };
}

identity!(ObjectId, "A semantic application-object identity.");
identity!(
    TableId,
    "A semantic table identity, separate from object IDs."
);
identity!(
    OperationId,
    "A logical operation identity, separate from record IDs."
);
identity!(
    CursorId,
    "A cursor identity issued by the session that owns it."
);

/// A caller-issued source/session generation that namespaces record locators.
///
/// Owners must issue a new generation when locators become invalid. This is not
/// a transaction revision, wall clock, or claim of backend snapshot isolation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceGeneration(u64);

impl SourceGeneration {
    /// Wrap an owner-issued generation.
    pub const fn new(generation: u64) -> Self {
        Self(generation)
    }

    /// Read the generation token.
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_tokens_survive_explicit_round_trips_without_numeric_truncation() {
        for token in [0, 1, u64::MAX as u128 + 1, u128::MAX] {
            assert_eq!(ObjectId::from_u128(token).as_u128(), token);
            assert_eq!(TableId::from_u128(token).as_u128(), token);
            assert_eq!(OperationId::from_u128(token).as_u128(), token);
            assert_eq!(CursorId::from_u128(token).as_u128(), token);
        }
        assert_eq!(SourceGeneration::new(u64::MAX).get(), u64::MAX);
    }
}
