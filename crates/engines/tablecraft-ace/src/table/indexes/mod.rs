mod definition;
mod header;
mod metadata;
pub use metadata::{IndexColumn, IndexInspection, NativeIndex};

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
mod page;
mod read;
mod walk;
pub use read::read_table_index;
