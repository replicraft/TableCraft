//! Atomic fixed-field batch preview composition.
mod plan;

mod split;

#[cfg(test)]
pub(in crate::table) mod multitable_tests;
#[cfg(test)]
pub(in crate::table) mod split_tests;
