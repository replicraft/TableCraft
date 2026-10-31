//! Atomic fixed-field batch preview composition.
mod plan;
mod request;
mod split;

pub use request::FixedColumnUpdates;

#[cfg(test)]
pub(in crate::table) mod multitable_tests;
#[cfg(test)]
pub(in crate::table) mod split_tests;
