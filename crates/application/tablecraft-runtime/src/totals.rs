//! Derived totals over a complete checked native view, independent of page rows.
mod service;
mod snapshot;

use crate::{Cell, Column};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TotalsTarget {
    pub document_revision: u64,
    pub table: String,
    pub filter_id: Option<u64>,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TotalAggregate {
    Count,
    Sum,
    Average,
    Minimum,
    Maximum,
    StandardDeviation,
    Variance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TotalSpec {
    pub column: String,
    pub aggregate: TotalAggregate,
}

pub struct TotalsField {
    pub column: Column,
    pub aggregates: Vec<TotalAggregate>,
}

pub struct TotalsDescription {
    pub target: TotalsTarget,
    pub fields: Vec<TotalsField>,
}

/// These cells are derived values and carry no original row or edit identity.
pub struct TotalsData {
    pub target: TotalsTarget,
    pub specs: Vec<TotalSpec>,
    pub cells: Vec<Cell>,
    pub source_rows: u32,
    pub view_rows: u32,
}

#[derive(Debug, Error)]
pub enum TotalsError {
    #[error("totals target does not match the current native view")]
    InvalidTarget,
    #[error("totals fields must fit the observed table schema")]
    FieldLimit,
    #[error("the total is not supported for this native field")]
    UnsupportedAggregate,
    #[error("native totals source membership changed")]
    MembershipChanged,
}

impl TotalAggregate {
    fn function(self) -> &'static str {
        match self {
            Self::Count => "Count",
            Self::Sum => "Sum",
            Self::Average => "Avg",
            Self::Minimum => "Min",
            Self::Maximum => "Max",
            Self::StandardDeviation => "StDev",
            Self::Variance => "Var",
        }
    }
}
