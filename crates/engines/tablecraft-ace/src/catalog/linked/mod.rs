//! Explicit inert native linked-table definitions, without target discovery.
mod fields;
mod metadata;
mod read;

pub use metadata::{NativeAccessLinkTarget, NativeLinkText, NativeLinkedTableDefinition};
pub use read::read_linked_table_definition;
