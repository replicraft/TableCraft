//! Engineering-only independent raw fixture builders.
//!
//! This package has no dependency on a production engine, adapter or its byte
//! layouts. Literal test bytes remain independent of production constants.
//! Workspace consumers may depend on it only for development and testing.
pub mod access;
