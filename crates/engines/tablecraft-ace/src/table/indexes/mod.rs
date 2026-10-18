mod constraints;
mod definition;
pub use constraints::{IndexConstraintViolation, IndexConstraints};
mod header;
mod metadata;
mod profile;
pub use metadata::{IndexColumn, IndexInspection, NativeIndex};
use profile::integer_column;
mod check;
pub use check::{IndexCheck, IndexCheckStatus, TableCheck, check_table};

use super::schema::inspect_table_schema;
use crate::{InspectError, format::reader::Reader};
use std::io::{Read, Seek};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

/// Inspect native logical/physical index metadata for one explicit local table.
/// Does not traverse index/data pages or execute active content.
pub fn inspect_table_indexes<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<IndexInspection, InspectError> {
    let _span = tracing::debug_span!("native.index.inspect").entered();
    let schema = inspect_table_schema(source, scope, name, budget, cancelled)?;
    let mut reader = Reader {
        source,
        header: schema.header,
        budget,
        cancelled,
        reads: 0,
    };
    let indexes = definition::read(&mut reader, &schema)?;
    reader.tick()?;
    Ok(IndexInspection { schema, indexes })
}
mod mutation;
mod page;
pub(super) use mutation::{Int32KeyUpdate, patches as index_patches};
mod read;
mod selection;
mod walk;
pub use read::{read_table_index, read_table_index_range};
pub use selection::Int32IndexSelection;
