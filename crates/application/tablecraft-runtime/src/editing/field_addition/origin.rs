//! Captured field policy origins stay distinct from gallery input declarations.
use tablecraft_builders::table::FieldAdditionDeclaration;
use tablecraft_schema::ScalarType;

/// Inert declaration facts captured by the private native review plan.
/// Constructing these facts cannot issue a mutation or reopen capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldAdditionOrigin {
    /// The existing design/gallery declaration with its explicit native policies.
    Gallery(FieldAdditionDeclaration),
    /// Unadorned SQL declaration, with Required/AZL/DefaultValue absent.
    /// Current native SQL ADD qualification is Currency only. Other values do
    /// not grant availability or bypass the exact native declaration proof.
    SqlDeclared {
        /// Actual scalar type from the checked native declared request.
        scalar_type: ScalarType,
    },
}
