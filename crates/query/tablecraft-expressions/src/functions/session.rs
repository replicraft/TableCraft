//! Explicit immutable Access expression-session data, independent of permissions.
use crate::EvalError;

const UNSECURED_USER: &str = "Admin";

/// Caller-issued account name used by CurrentUser. This is expression metadata,
/// not proof of login, a security principal, or permission to access any object.
/// Debug/Serde are intentionally absent to avoid implicit account-name exposure.
#[derive(Clone, Copy)]
pub struct AccessSessionContext<'a> {
    user_name: &'a str,
}

impl<'a> AccessSessionContext<'a> {
    /// Maximum UTF-8 bytes admitted by this host-data profile, not a native MDW
    /// account-name limit. Evaluation may impose a narrower Text ceiling.
    pub const MAX_NAME_BYTES: usize = 65_536;

    /// Declare an unsecured Access expression session with the default account.
    /// This does not authenticate against an MDB workgroup or grant permissions.
    pub const fn unsecured() -> Self {
        Self {
            user_name: UNSECURED_USER,
        }
    }

    /// Retain a host-supplied account name without trimming, case folding or
    /// normalization. Empty/control-containing names are outside this profile.
    pub fn new(user_name: &'a str) -> Result<Self, EvalError> {
        if user_name.len() > Self::MAX_NAME_BYTES {
            return Err(EvalError::TextLimit);
        }
        if user_name.is_empty() || user_name.chars().any(char::is_control) {
            return Err(EvalError::InvalidSessionName);
        }
        Ok(Self { user_name })
    }

    /// Borrow the exact supplied session account name; no provider is consulted.
    pub const fn current_user(self) -> &'a str {
        self.user_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_data_is_explicit_and_preserves_exact_names() {
        assert_eq!(AccessSessionContext::unsecured().current_user(), "Admin");
        for name in ["Reader", "é中🦀", " e\u{301} ", "admin", "Admin"] {
            assert_eq!(
                AccessSessionContext::new(name).unwrap().current_user(),
                name
            );
        }
        for name in ["", "reader\0", "reader\n", "\treader"] {
            assert!(matches!(
                AccessSessionContext::new(name),
                Err(EvalError::InvalidSessionName)
            ));
        }
        assert!(matches!(
            AccessSessionContext::new(&"x".repeat(65537)),
            Err(EvalError::TextLimit)
        ));
        assert!(AccessSessionContext::new(&"x".repeat(65536)).is_ok());
    }
}
