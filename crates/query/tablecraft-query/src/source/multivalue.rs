//! Query visitors over the shared borrowed scalar collection contract.
use crate::QueryError;
use tablecraft_db::{MultivalueList, ProjectedRow};

/// One parent scalar projection and its requested borrowed collection lists.
pub type MultivalueVisitor<'a> =
    dyn FnMut(ProjectedRow<'_>, &[&dyn MultivalueList]) -> Result<(), QueryError> + 'a;
