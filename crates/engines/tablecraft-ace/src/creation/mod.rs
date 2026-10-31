//! Bounded fresh ACE 16 database serialization, separate from edit authority.
mod build;
mod column;
mod database;
mod definition;
mod input;
mod profile;
mod record;
mod request;
mod stored_text;
mod variable;

pub use column::CreateColumn;
pub use database::{CreatedDatabase, create_ace16_database, create_ace16_int32_database};
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
