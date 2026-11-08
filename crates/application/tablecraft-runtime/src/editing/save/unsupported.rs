//! Unsupported hosts return every unconsumed capability without starting work.
use super::{SaveReport, SaveStartError, SaveStartFailure};
#[cfg(feature = "ace16-create")]
use crate::editing::ReviewedTableCreation;
use crate::editing::{
    ReviewedDelete, ReviewedEdit, ReviewedInsert, ReviewedQueryDelete, ReviewedQueryUpdate,
};
use std::path::PathBuf;

pub struct SaveWorker;

impl SaveWorker {
    #[cfg(feature = "ace16-create")]
    pub const fn supports_table_creation() -> bool {
        false
    }

    pub fn new(_repaint: impl Fn() + Send + Sync + 'static) -> Self {
        Self
    }

    pub fn start(&self, reviewed: ReviewedEdit, _bundle: PathBuf) -> Result<u64, SaveStartError> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: Some(reviewed),
        })
    }

    pub fn start_insert(
        &self,
        reviewed: ReviewedInsert,
        _bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedInsert>> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: Some(reviewed),
        })
    }

    pub fn start_delete(
        &self,
        reviewed: ReviewedDelete,
        _bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedDelete>> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: Some(reviewed),
        })
    }

    pub fn start_query_update(
        &self,
        reviewed: ReviewedQueryUpdate,
        _bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedQueryUpdate>> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: Some(reviewed),
        })
    }

    pub fn start_query_delete(
        &self,
        reviewed: ReviewedQueryDelete,
        _bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedQueryDelete>> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: Some(reviewed),
        })
    }

    #[cfg(feature = "ace16-create")]
    pub fn start_table_creation(
        &self,
        reviewed: ReviewedTableCreation,
        _bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedTableCreation>> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: Some(reviewed),
        })
    }

    pub fn poll(&self) -> Option<SaveReport> {
        None
    }

    pub fn cancel(&self, _id: u64) -> bool {
        false
    }

    pub fn acknowledge(&self, _id: u64) -> bool {
        false
    }

    pub fn start_reserved_recovery(&self, source: PathBuf) -> Result<u64, SaveStartError<PathBuf>> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: Some(source),
        })
    }

    pub fn recover(&self, _id: u64) -> Result<u64, SaveStartError> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: None,
        })
    }

    pub fn release(&self, _id: u64) -> Result<u64, SaveStartError> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: None,
        })
    }

    pub fn is_busy(&self) -> bool {
        false
    }
}
