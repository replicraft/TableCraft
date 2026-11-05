//! Immutable, source-issued table filters; presentation indices are not identities.
mod criteria;
mod definition;
mod page;
mod scan;
mod service;

use crate::{Cell, Column, Error};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tablecraft_core::values::Value;
use tablecraft_schema::TypeDescriptor;
use thiserror::Error as ThisError;

pub const MAX_FILTER_FIELDS: usize = 16;
const RETAINED_BYTES: u64 = 8 * 1024 * 1024;
const FILTER_STEPS: u64 = 1_000_000;
static NEXT_FILTER_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterTarget {
    pub document_revision: u64,
    pub table: String,
    pub filter_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterSelectionTarget {
    pub document_revision: u64,
    pub table: String,
    pub parent_ordinal: u32,
    pub column: String,
    pub filter_id: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterComparison {
    Equal,
    NotEqual,
}

/// A read-only issued capability. Cloning shares its immutable map and criteria.
/// UI DTOs cannot construct one or turn displayed text into a stored operand.
#[derive(Clone)]
pub struct FilterHandle {
    state: Arc<FilterState>,
}

/// Complete bounded page publication with its original native row positions.
pub struct FilteredData {
    pub handle: FilterHandle,
    pub target: FilterTarget,
    pub enabled: bool,
    pub offset: u32,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    pub source_ordinals: Vec<u32>,
    pub matched_rows: u32,
    pub source_rows: u32,
    pub has_more: bool,
}

/// Captured typed filter preferences, with no row map or source capability.
#[derive(Clone)]
pub struct FilterDefinition {
    state: Arc<DefinitionState>,
}

struct DefinitionState {
    table: String,
    criteria: Vec<Criterion>,
}

impl FilterDefinition {
    pub fn table(&self) -> &str {
        &self.state.table
    }
}

struct FilterState {
    source: Arc<()>,
    target: FilterTarget,
    definition: FilterDefinition,
    ordinals: Vec<u32>,
    source_rows: u32,
}

struct Criterion {
    column: String,
    declaration: TypeDescriptor,
    predicate: Predicate,
}

enum Predicate {
    Equal(Value),
    NotEqual(Value),
    IsNull,
    IsNotNull,
}

impl FilterHandle {
    pub fn target(&self) -> &FilterTarget {
        &self.state.target
    }

    pub fn matched_rows(&self) -> u32 {
        self.state.ordinals.len() as u32
    }

    pub fn source_rows(&self) -> u32 {
        self.state.source_rows
    }

    pub fn definition(&self) -> FilterDefinition {
        self.state.definition.clone()
    }

    pub(crate) fn ordinals(&self) -> &[u32] {
        &self.state.ordinals
    }

    pub fn display_offset_for_ordinal(&self, enabled: bool, ordinal: u32) -> Option<u32> {
        if enabled {
            self.state
                .ordinals
                .binary_search(&ordinal)
                .ok()
                .and_then(|position| u32::try_from(position).ok())
        } else {
            (ordinal < self.state.source_rows).then_some(ordinal)
        }
    }
}

#[derive(Debug, ThisError)]
pub enum FilterError {
    #[error("the filter handle belongs to another native source or revision")]
    StaleHandle,
    #[error("the selected native field or record is not part of this filter source")]
    InvalidSelection,
    #[error("filter criteria exceed the sixteen distinct-field limit")]
    CriterionLimit,
    #[error("the selected field does not support this scalar filter")]
    UnsupportedField,
    #[error("filter identifier counter is exhausted")]
    IdentifierExhausted,
}

fn issue_id() -> Result<u64, Error> {
    NEXT_FILTER_ID
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map(|id| id + 1)
        .map_err(|_| FilterError::IdentifierExhausted.into())
}
