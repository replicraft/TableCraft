//! Fresh image ownership and the public bounded creation operation.
use super::{
    CreateColumn, CreateDatabaseError, build,
    input::{Columns, Rows},
    multiple::{self, CreateTable},
    request,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_core::values::Value;

/// An independently serialized fresh database image in the bounded declared-scalar profile.
///
/// This artifact grants no existing-file mutation, macro-absence, journal or
/// native Access save authority. Header fields outside the explicitly authored
/// profile are zero choices, not a claim that every such field is reserved.
pub struct CreatedDatabase {
    bytes: Vec<u8>,
    rows: usize,
    columns: usize,
    tables: usize,
}

impl CreatedDatabase {
    /// Complete serialized file bytes, without filesystem publication.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Total complete rows across the created user tables.
    pub const fn row_count(&self) -> usize {
        self.rows
    }

    /// Total declared scalar columns across the created user tables.
    pub const fn column_count(&self) -> usize {
        self.columns
    }

    /// Number of user tables; explicit blank creation returns zero.
    pub const fn table_count(&self) -> usize {
        self.tables
    }
}

/// Serialize a fresh, unencrypted ACE 16 image containing only the authored
/// system catalog and definitions, with no user tables, fields or rows.
///
/// This is direct bounded construction, not a template copy or existing-file
/// mutation. It preserves distinct catalog Memo/OLE allocation maps and conveys
/// no native Access open/save qualification or future edit permission.
pub fn create_blank_ace16_database(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CreatedDatabase, CreateDatabaseError> {
    let _span = tracing::debug_span!("native.database.create_blank").entered();
    let captured = multiple::capture_blank(budget, cancelled)?;
    let bytes = multiple::build(&captured, budget, cancelled)?;
    crate::header::check(budget, cancelled)?;

    Ok(CreatedDatabase {
        bytes,
        rows: 0,
        columns: 0,
        tables: 0,
    })
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
    create(
        table,
        Columns::Int32(columns),
        Rows::Int32(rows),
        budget,
        cancelled,
    )
}

/// Serialize a fresh ACE 16 database from explicit native declarations.
///
/// The qualified families are Boolean, Byte, Int16, Int32, Int64, Currency,
/// Single, Double, legacy DateTime and Guid. Each non-NULL cell must have its
/// exact declared carrier; Boolean requires a Boolean value and has no NULL
/// payload. Short Text and Short Binary require explicit positive UTF-16-unit
/// and byte capacities through their declaration factories. Decimal requires
/// explicit precision and exact retained scale; values are never rescaled.
/// Scalar-only variable declarations and extended families refuse even for
/// empty or all-NULL input. No coercion, generated identity,
/// implicit Required declaration, existing-file edit or journal authority is inferred.
/// Explicit CreateColumn Boolean policies are validated and serialized as field properties.
/// The same name, count and physical-fit bounds as the Int32 convenience apply.
pub fn create_ace16_database(
    table: &str,
    columns: &[CreateColumn<'_>],
    rows: &[&[Value]],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CreatedDatabase, CreateDatabaseError> {
    create(
        table,
        Columns::Values(columns),
        Rows::Values(rows),
        budget,
        cancelled,
    )
}

fn create<'a>(
    table: &'a str,
    columns: Columns<'a>,
    rows: Rows<'a>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CreatedDatabase, CreateDatabaseError> {
    let _span = tracing::debug_span!("native.database.create").entered();
    let request = request::validate(table, columns, rows, budget, cancelled)?;
    let bytes = build::build(&request, budget, cancelled)?;
    crate::header::check(budget, cancelled)?;

    Ok(CreatedDatabase {
        bytes,
        rows: rows.count(),
        columns: columns.count(),
        tables: 1,
    })
}

/// Serialize one fresh ACE 16 image containing one through sixteen user tables.
///
/// Each borrowed table retains explicit native declarations and exact Value rows.
/// Table names are distinct under ASCII folding and share the existing name
/// bounds. Every table must independently fit its one DATA page with at most
/// sixteen columns and sixty-four rows. All tables and actual catalog/map layout
/// bounds are validated before image allocation, under one cumulative budget.
/// No relationship, generated field, implicit constraint property, existing-file edit,
/// required-event absence or native Access save permission is inferred.
/// Artifact row/column counts are totals; table_count reports the user count.
pub fn create_ace16_database_tables(
    tables: &[CreateTable<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CreatedDatabase, CreateDatabaseError> {
    let _span = tracing::debug_span!("native.database.create_tables").entered();
    let captured = multiple::capture(tables, budget, cancelled)?;
    let bytes = multiple::build(&captured, budget, cancelled)?;
    crate::header::check(budget, cancelled)?;

    Ok(CreatedDatabase {
        bytes,
        rows: captured.rows,
        columns: captured.columns,
        tables: tables.len(),
    })
}
