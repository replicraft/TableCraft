//! Captured field policy origins stay distinct from gallery input declarations.
use tablecraft_builders::table::{FieldAdditionDeclaration, TypedFieldAdditionDeclaration};

/// Inert declaration facts captured by the private native review plan.
/// Constructing these facts cannot issue a mutation or reopen capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldAdditionOrigin {
    /// The existing design/gallery declaration with its explicit native policies.
    Gallery(FieldAdditionDeclaration),
    /// Exact scalar shape from the new unadorned SQL native request. This is
    /// declaration data only: gallery Required/AZL/default policies do not apply.
    SqlUnadorned(TypedFieldAdditionDeclaration),
}
