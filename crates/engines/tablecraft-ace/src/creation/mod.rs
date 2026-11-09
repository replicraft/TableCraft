//! Bounded fresh ACE 16 database serialization, separate from edit authority.
mod add_column;
#[cfg(feature = "journal")]
mod add_table;
mod autonumber;
mod build;
#[cfg(feature = "journal")]
mod catalog_page;
mod column;
mod database;
#[cfg(feature = "journal")]
mod database_options;
#[cfg(feature = "journal")]
pub(crate) use database_options::FieldHistoryOffSource;
mod defaults;
mod definition;
mod first_table;
mod input;
mod multiple;
mod primary;
mod profile;
mod properties;
mod record;
mod request;
mod stored_text;
mod variable;

pub use add_column::{AddColumnCandidate, AddColumnError};
#[cfg(feature = "journal")]
pub use add_column::{CreatedAddColumnCandidate, CreatedAddColumnError};
#[cfg(feature = "journal")]
pub use add_column::{
    CreatedDefaultZeroAddColumnCandidate, CreatedDefaultZeroAddColumnError,
    CreatedDefaultZeroColumnRequired, CreatedFieldAdditionSourcePurpose,
    FieldAdditionAfterOffCandidate, FieldAdditionAfterOffError,
    FieldAdditionAfterOffHistoryCandidate, FieldAdditionAfterOffHistoryError,
    FieldAdditionCandidate, FieldAdditionError, FieldAdditionSourceAvailability,
    FieldAdditionSourceError, FieldAdditionSourcePurpose, FieldAdditionTarget,
    RepeatedDefaultZeroAddColumnCandidate,
};
#[cfg(feature = "journal")]
pub use add_table::{
    AddTableError, AddTablePlan, AddTableSourceCapabilities, add_table_source_available,
    add_table_source_capabilities,
};
pub use column::CreateColumn;
pub use database::{
    CreatedDatabase, create_ace16_database, create_ace16_database_tables,
    create_ace16_int32_database, create_blank_ace16_database,
};
#[cfg(feature = "journal")]
pub use database_options::{
    DatabaseOptionsError, DatabaseOptionsSourceError, DatabaseOptionsSourcePurpose,
    FieldHistoryDatabaseOptionsCandidate, FieldHistoryDatabaseOptionsError,
    NativeDatabaseOptionsPlan,
};
pub use first_table::{
    FirstTableDefinition, FirstTableError, FirstTablePlan, first_table_source_available,
};
pub use multiple::CreateTable;
pub use request::CreateDatabaseError;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod fixed_tests;

#[cfg(test)]
mod catalog_name_tests;

#[cfg(test)]
mod variable_tests;

#[cfg(test)]
mod capacity_tests;

#[cfg(test)]
mod property_tests;

#[cfg(test)]
mod default_tests;

#[cfg(test)]
mod primary_tests;

#[cfg(test)]
mod blank_tests;

#[cfg(test)]
mod description_tests;
