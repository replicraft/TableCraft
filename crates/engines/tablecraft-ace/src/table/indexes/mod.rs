mod boolean;
mod composite;
mod constraints;
mod decimal;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use decimal::DecimalType;
mod definition;
mod floating;
mod guid;
mod key;
mod legacy_datetime;
mod text;
pub use constraints::{IndexConstraintViolation, IndexConstraints};
mod header;
mod metadata;
mod profile;
pub use metadata::{IndexColumn, IndexInspection, NativeIndex};
mod reference;
pub(super) use reference::FOREIGN_INDEX;
pub use reference::ForeignIndexReference;
mod catalog;
mod links;
pub use links::{ForeignIndexInspection, ForeignIndexLink, inspect_foreign_index_links};
mod check;
pub use check::{IndexCheck, IndexCheckStatus, TableCheck, check_table};

use super::schema::inspect_table_schema;
use crate::{InspectError, format::reader::Reader};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

/// Inspect native logical/physical index metadata for one explicit local table.
/// Does not traverse index/data pages or execute active content.
pub fn inspect_table_indexes<R: crate::NativeReadSource>(
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
mod binary;
pub use binary::BinaryIndexSelection;
pub use read::read_table_binary_index_range;
mod mutation;
mod page;
pub(super) use mutation::{IndexDataPatch, IndexKeyUpdate, MutationKey, patches as index_patches};
mod read;
mod selection;
mod walk;
pub use read::{read_table_index, read_table_index_range, read_table_int64_index_range};
pub use selection::{Int32IndexSelection, Int64IndexSelection};

mod projection;
pub use projection::{IndexProjection, IndexProjectionSelection};
pub use read::read_table_index_projection;
