//! Multiple table capture and one complete image, separate from edit authority.
mod build;
mod layout;
mod request;

pub(super) use build::build;
pub use request::CreateTable;
pub(super) use request::capture;

#[cfg(test)]
mod tests;
