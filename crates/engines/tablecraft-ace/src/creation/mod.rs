//! Bounded fresh ACE 16 database serialization, separate from edit authority.
#[cfg(feature = "journal")]
mod add_table;
mod autonumber;
mod build;
mod column;
mod database;
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

#[cfg(feature = "journal")]
pub use add_table::{AddTableError, AddTablePlan, add_table_source_available};
pub use column::CreateColumn;
pub use database::{
    CreatedDatabase, create_ace16_database, create_ace16_database_tables,
    create_ace16_int32_database, create_blank_ace16_database,
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
