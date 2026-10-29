//! Atomic fixed-field batch preview composition.
mod plan;
mod request;
mod split;

pub use request::FixedColumnUpdates;

#[cfg(test)]
mod split_tests;
