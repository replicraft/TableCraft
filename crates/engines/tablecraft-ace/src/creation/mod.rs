//! Bounded fresh ACE 16 database serialization, separate from edit authority.
mod build;
mod column;
mod database;
mod defaults;
mod definition;
mod input;
mod multiple;
mod primary;
mod profile;
mod properties;
mod record;
mod request;
mod stored_text;
mod variable;

pub use column::CreateColumn;
pub use database::{
    CreatedDatabase, create_ace16_database, create_ace16_database_tables,
    create_ace16_int32_database,
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
