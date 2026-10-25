mod inventory;
mod linked;
mod origin;
mod record;
pub use inventory::{CatalogInspection, inspect_catalog};
pub use origin::{NativeObjectOrigin, NativeTableStorage};

pub use linked::{
    NativeAccessLinkTarget, NativeLinkText, NativeLinkedTableDefinition,
    read_linked_table_definition,
};
