mod declaration;
mod discovery;
mod metadata;
mod read;
mod reader;
mod record;
pub use reader::{ParentVersionHistory, VersionHistoryReader};
pub(in crate::table::complex) mod schema;
pub use declaration::VersionHistoryMemberDeclaration;
pub use discovery::inspect_version_history_declarations;
pub use metadata::{StoredVersion, VersionHistoryColumn};
pub use read::{read_version_history_column, read_version_history_column_with_limits};

pub(in crate::table::complex) use read::read_links;
pub(in crate::table::complex) use schema::select;
