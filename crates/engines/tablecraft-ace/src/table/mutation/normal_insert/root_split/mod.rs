//! Complete original-row root promotion; physical and proof owners stay separate.
mod admission;
mod authority;
mod proof;
mod selection;
mod stage;

pub(super) use selection::selected;
pub(super) use stage::plan;

#[cfg(test)]
mod tests;
