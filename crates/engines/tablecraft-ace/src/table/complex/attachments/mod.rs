mod declaration;
pub use declaration::AttachmentMemberDeclaration;
mod discovery;
pub use discovery::{inspect_attachment_declarations, inspect_attachment_members};
mod metadata;
mod read;
mod record;
mod schema;
pub use metadata::{AttachmentColumn, StoredAttachment};
pub use read::{read_attachment_column, read_attachment_column_with_limits};

pub(in crate::table::complex) use read::read_links;
