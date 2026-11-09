//! Borrowed typed and legacy Int32 inputs share a single physical builder.
use super::{CreateColumn, column::Declaration};
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy)]
pub(super) enum Columns<'a> {
    Int32(&'a [&'a str]),
    Values(&'a [CreateColumn<'a>]),
}

impl<'a> Columns<'a> {
    pub fn count(self) -> usize {
        match self {
            Self::Int32(columns) => columns.len(),
            Self::Values(columns) => columns.len(),
        }
    }

    pub fn name(self, position: usize) -> &'a str {
        match self {
            Self::Int32(columns) => columns[position],
            Self::Values(columns) => columns[position].name(),
        }
    }

    pub fn description(self, position: usize) -> Option<&'a str> {
        match self {
            Self::Int32(_) => None,
            Self::Values(columns) => columns[position].description(),
        }
    }

    pub fn scalar(self, position: usize) -> ScalarType {
        self.declaration(position).scalar()
    }

    pub fn policies(self, position: usize) -> (Option<bool>, Option<bool>) {
        match self {
            Self::Int32(_) => (None, None),
            Self::Values(columns) => (
                columns[position].required(),
                columns[position].allow_zero_length(),
            ),
        }
    }

    pub fn default_literal(self, position: usize) -> Option<&'a Value> {
        match self {
            Self::Int32(_) => None,
            Self::Values(columns) => columns[position].default_literal(),
        }
    }

    pub fn primary_key_name(self, position: usize) -> Option<&'a str> {
        match self {
            Self::Int32(_) => None,
            Self::Values(columns) => columns[position].primary_key_name(),
        }
    }

    pub fn incrementing(self, position: usize) -> bool {
        match self {
            Self::Int32(_) => false,
            Self::Values(columns) => columns[position].incrementing_autonumber(),
        }
    }

    pub fn unicode_compression(self, position: usize) -> bool {
        match self {
            Self::Int32(_) => false,
            Self::Values(columns) => columns[position].unicode_compression(),
        }
    }

    pub fn declaration(self, position: usize) -> Declaration {
        match self {
            Self::Int32(_) => Declaration::Fixed(ScalarType::Int32),
            Self::Values(columns) => columns[position].declaration(),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Rows<'a> {
    Int32(&'a [&'a [i32]]),
    Values(&'a [&'a [Value]]),
}

impl<'a> Rows<'a> {
    pub fn count(self) -> usize {
        match self {
            Self::Int32(rows) => rows.len(),
            Self::Values(rows) => rows.len(),
        }
    }

    pub fn width(self, row: usize) -> usize {
        match self {
            Self::Int32(rows) => rows[row].len(),
            Self::Values(rows) => rows[row].len(),
        }
    }

    pub fn cell(self, row: usize, column: usize) -> Cell<'a> {
        match self {
            Self::Int32(rows) => Cell::Int32(rows[row][column]),
            Self::Values(rows) => Cell::Value(&rows[row][column]),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Cell<'a> {
    Int32(i32),
    Value(&'a Value),
}
