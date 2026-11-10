//! Immutable, source-issued table filters; presentation indices are not identities.
mod criteria;
mod definition;
mod field;
mod page;
mod patterns;
mod scan;
mod service;
mod sorting;

pub use field::{
    FieldFilterDescription, FieldFilterOperator, FieldFilterPatternMode, FieldFilterTarget,
    MAX_FIELD_FILTER_INPUT_BYTES,
};

use crate::{Cell, Column, Error};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tablecraft_core::values::Value;
use tablecraft_expressions::comparison::Comparison;
use tablecraft_schema::TypeDescriptor;
use thiserror::Error as ThisError;

const RETAINED_BYTES: u64 = u64::MAX;
const FILTER_STEPS: u64 = u64::MAX;
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
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterComparison {
    Equal,
    NotEqual,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortTarget {
    pub document_revision: u64,
    pub table: String,
    pub filter_id: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortSpec {
    pub column: String,
    pub descending: bool,
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
    pub sort: Option<SortSpec>,
    pub filter_count: u32,
}

/// Captured typed filter preferences, with no row map or source capability.
#[derive(Clone)]
pub struct FilterDefinition {
    state: Arc<DefinitionState>,
}

struct DefinitionState {
    table: String,
    criteria: Vec<Criterion>,
    sort: Option<BoundSort>,
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
    order: Option<DisplayOrder>,
}

struct BoundSort {
    spec: SortSpec,
    declaration: TypeDescriptor,
}

struct DisplayOrder {
    enabled: bool,
    ordinals: Vec<u32>,
    ranks: Vec<Option<u32>>,
}

struct Criterion {
    column: String,
    declaration: TypeDescriptor,
    predicate: Predicate,
}

enum Predicate {
    Equal(Value),
    NotEqual(Value),
    Ordered {
        comparison: Comparison,
        expected: Value,
    },
    Between {
        first: Value,
        second: Value,
        negated: bool,
    },
    Pattern {
        operator: FieldFilterOperator,
        input: String,
    },
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
        // NO_CLONE: The public definition snapshot is owned independently of the retained
        // filter handle so hosts can preserve inert restore preferences after native
        // source invalidation.
        self.state.definition.clone()
    }

    pub(crate) fn ordinals(&self) -> &[u32] {
        &self.state.ordinals
    }

    pub fn sort(&self) -> Option<&SortSpec> {
        self.state
            .definition
            .state
            .sort
            .as_ref()
            .map(|sort| &sort.spec)
    }

    pub fn filter_count(&self) -> u32 {
        self.state.definition.state.criteria.len() as u32
    }

    pub(crate) fn mode_cached(&self, enabled: bool) -> bool {
        self.state
            .order
            .as_ref()
            .is_none_or(|order| order.enabled == enabled)
    }

    fn display_ordinal(&self, enabled: bool, offset: u32) -> Option<u32> {
        if let Some(order) = &self.state.order {
            if order.enabled != enabled {
                return None;
            }
            order.ordinals.get(offset as usize).copied()
        } else if enabled {
            self.state.ordinals.get(offset as usize).copied()
        } else {
            (offset < self.state.source_rows).then_some(offset)
        }
    }

    pub fn display_offset_for_ordinal(&self, enabled: bool, ordinal: u32) -> Option<u32> {
        if let Some(order) = &self.state.order {
            if order.enabled != enabled {
                return None;
            }
            order.ranks.get(ordinal as usize).copied().flatten()
        } else if enabled {
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
    #[error("filter criteria exceed the observed table field count")]
    CriterionLimit,
    #[error("the field does not support this common-filter operator")]
    UnsupportedOperator,
    #[error("the filter operator requires a different number of operands")]
    InvalidOperands,
    #[error("a filter operand exceeds the 64 KiB UTF-8 input ceiling")]
    InputLimit,
    #[error("the database wildcard mode is not qualified")]
    UnknownPatternMode,
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
