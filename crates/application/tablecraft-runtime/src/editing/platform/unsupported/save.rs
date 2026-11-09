//! Unsupported hosts return every unconsumed capability without starting work.
use crate::editing::save::{SaveReport, SaveStartError, SaveStartFailure};
#[cfg(feature = "ace16-create")]
use crate::editing::{
    ReviewedAddColumn, ReviewedDatabaseOptions, ReviewedNumberColumn, ReviewedTableCreation,
};
use crate::editing::{
    ReviewedDelete, ReviewedEdit, ReviewedInsert, ReviewedQueryDelete, ReviewedQueryExpression,
    ReviewedQueryInsert, ReviewedQuerySql, ReviewedQueryUpdate, ReviewedTableRename,
};
use std::path::PathBuf;

pub struct SaveWorker;

impl SaveWorker {
    #[cfg(feature = "ace16-create")]
    pub const fn supports_database_options() -> bool {
        false
    }

    #[cfg(feature = "ace16-create")]
    pub fn start_disable_name_auto_correct(
        &self,
        reviewed: ReviewedDatabaseOptions,
        _bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedDatabaseOptions>> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: Some(reviewed),
        })
    }

    pub const fn supports_query_expression() -> bool {
        false
    }

    /// Full-SQL saves remain explicitly unavailable on this host.
    pub const fn supports_query_sql() -> bool {
        false
    }

    /// Return the exact unconsumed capability without starting native work.
    pub fn start_query_sql(
        &self,
        reviewed: ReviewedQuerySql,
        _bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedQuerySql>> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: Some(reviewed),
        })
    }

    pub fn start_query_expression(
        &self,
        reviewed: ReviewedQueryExpression,
        _bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedQueryExpression>> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: Some(reviewed),
        })
    }

    #[cfg(feature = "ace16-create")]
    pub const fn supports_number_column() -> bool {
        false
    }

    #[cfg(feature = "ace16-create")]
    pub const fn supports_add_column() -> bool {
        false
    }

    pub const fn supports_table_rename() -> bool {
        false
    }

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

    pub fn start_query_insert(
        &self,
        reviewed: ReviewedQueryInsert,
        _bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedQueryInsert>> {
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

    #[cfg(feature = "ace16-create")]
    pub fn start_number_column(
        &self,
        reviewed: ReviewedNumberColumn,
        _bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedNumberColumn>> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: Some(reviewed),
        })
    }

    #[cfg(feature = "ace16-create")]
    pub fn start_add_column(
        &self,
        reviewed: ReviewedAddColumn,
        _bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedAddColumn>> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: Some(reviewed),
        })
    }

    pub fn start_table_rename(
        &self,
        reviewed: ReviewedTableRename,
        _bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedTableRename>> {
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
