//! Development-only independent fixtures and shared test machinery.
//!
//! Raw fixtures use core carriers and keep their literal byte expectations
//! independent of production codecs. Optional support modules observe real
//! subsystem APIs. Workspace consumers use this crate only in dev-dependencies.
pub mod fixtures;
pub mod support;
