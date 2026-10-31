//! Bounded fresh ACE 16 database serialization, separate from edit authority.
mod build;
mod database;
mod definition;
mod profile;
mod record;
mod request;

pub use database::{CreatedDatabase, create_ace16_int32_database};
pub use request::CreateDatabaseError;

#[cfg(test)]
mod tests;
