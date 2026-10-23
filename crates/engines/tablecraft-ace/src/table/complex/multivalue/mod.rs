mod declaration;
pub use declaration::MultivalueDeclaration;
mod metadata;
mod page;
pub use page::{
    MultivaluePageCell, MultivaluePageValue, MultivalueTablePage, read_multivalue_table_page,
};
mod read;
mod schema;
pub use metadata::MultivalueColumn;
pub use read::{read_multivalue_column, read_multivalue_column_with_limits};
pub use schema::{inspect_multivalue_columns, inspect_multivalue_declarations};

pub(in crate::table::complex) use read::read_links;
