//! Atomic fixed-field batch preview composition.
mod plan;
mod request;

pub use request::FixedColumnUpdates;

#[cfg(test)]
mod split_tests;
