//! Shared vocabulary independent of database codecs, GUI and network runtimes.
//!
//! These are in-memory contracts, not Rust layouts to write into database files.
//! See the repository's `docs/CORE_TYPES.md` for sources and supported boundaries.
#![deny(missing_docs)]

pub mod ids;
pub mod limits;
pub mod values;
