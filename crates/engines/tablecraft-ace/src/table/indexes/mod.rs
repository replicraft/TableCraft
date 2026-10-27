mod boolean;
mod composite;
mod constraints;
mod currency;
pub use currency::CurrencyIndexSelection;
mod decimal;
pub use decimal::DecimalIndexSelection;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use decimal::DecimalType;
mod definition;
pub(in crate::table) use definition::read_with_tail as definitions_with_tail;
mod floating;
mod guid;
pub use guid::GuidIndexSelection;
pub use read::read_table_guid_index;
mod key;
mod legacy_datetime;
pub use legacy_datetime::DateTimeIndexSelection;
mod numeric;
mod text;
pub use constraints::{IndexConstraintViolation, IndexConstraints};
mod encoded_tuple;
mod entry_digest;
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
pub use read::read_table_text_index_range;
pub use text::{GeneralEqualityPrefix, TextIndexSelection};
mod mutation;
mod page;
pub(super) use mutation::{IndexDataPatch, IndexKeyUpdate, MutationKey, patches as index_patches};
#[cfg(feature = "ace16-write-preview")]
pub(super) use mutation::{
    PatchSet as DeleteIndexPlan, RetirementTable, audit_global, audit_retirements, audit_siblings,
    compose as compose_retirements, delete_patches as index_delete_patches,
};
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use mutation::{
    SlotKeyUpdate, qualify_slot_indexes, slot_patches as index_slot_patches,
};
mod read;
mod selection;
mod walk;
pub use read::{read_table_index, read_table_index_range, read_table_int64_index_range};
pub use selection::{
    BooleanIndexSelection, ByteIndexSelection, Int16IndexSelection, Int32IndexSelection,
    Int64IndexSelection,
};

mod projection;
pub use projection::{IndexProjection, IndexProjectionSelection, IndexReadRequest};
pub use read::{
    read_table_index_in_table_order, read_table_index_projection, read_table_index_with_request,
};

#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use mutation::{SplitIndexPlan, splitting_patches as index_split_patches};
