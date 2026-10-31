//! Fresh image ownership and the public bounded creation operation.
use super::{CreateDatabaseError, build, request};
use tablecraft_core::limits::ResourceBudget;

/// An independently serialized fresh database image in the bounded Int32 profile.
///
/// This artifact grants no existing-file mutation, macro-absence, journal or
/// native Access save authority. Header fields outside the explicitly authored
/// profile are zero choices, not a claim that every such field is reserved.
pub struct CreatedDatabase {
    bytes: Vec<u8>,
    rows: usize,
    columns: usize,
}

impl CreatedDatabase {
    /// Complete serialized file bytes, without filesystem publication.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Number of rows in the single user table.
    pub const fn row_count(&self) -> usize {
        self.rows
    }

    /// Number of ordinary Int32 columns in the single user table.
    pub const fn column_count(&self) -> usize {
        self.columns
    }
}

/// Serialize one fresh header-code-5 ACE database containing one Int32 table.
///
/// Names must be nonempty and control-free, without leading ASCII space or the
/// native period, exclamation mark, grave accent or square brackets, within 64
/// UTF-16 units; duplicates use ASCII
/// folding. Names are preserved without normalization or silent coercion.
/// The profile admits 1..=16 columns and 0..=64 rectangular, present Int32 rows,
/// subject to actual single-page fit. It authors the catalog, system definitions,
/// complete declaration tails and represented map records directly. No source
/// database, active content, template, filesystem or native application is used.
///
/// This bounded fresh-creation capability is distinct from normal edits to an
/// existing database and does not qualify native Access open/save behavior.
pub fn create_ace16_int32_database(
    table: &str,
    columns: &[&str],
    rows: &[&[i32]],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CreatedDatabase, CreateDatabaseError> {
    let _span = tracing::debug_span!("native.database.create").entered();
    let request = request::validate(table, columns, rows, budget, cancelled)?;
    let bytes = build::build(&request, budget, cancelled)?;
    crate::header::check(budget, cancelled)?;

    Ok(CreatedDatabase {
        bytes,
        rows: rows.len(),
        columns: columns.len(),
    })
}
