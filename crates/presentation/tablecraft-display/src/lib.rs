//! Renderer-neutral regional display from validated facts; no I/O, SQL or GUI.
mod engine;
mod model;

pub use engine::{Engine, FormatError, raw};
pub use model::{
    CurrencyChoice, DateFormat, DateRepresentation, NativeFormat, Options, ScalarPresentation,
    TimeFormat,
};

#[cfg(test)]
mod tests;
