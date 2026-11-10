//! Complete record codecs shared by real normal and engineering consumers.
pub(super) mod build;
pub(super) mod fixed;
pub(super) mod layout;
pub(super) mod payload;
pub(super) mod proof;
pub(super) mod properties;

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(super) mod current;
