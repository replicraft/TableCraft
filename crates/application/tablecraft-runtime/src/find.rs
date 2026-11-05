//! Revision-bound native Find requests, independent of page publications and SQL.
mod matching;
mod native;

use tablecraft_display::Options;
use thiserror::Error;

pub const MAX_FIND_PATTERN_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindTarget {
    pub document_revision: u64,
    pub table: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindSpan {
    pub start_utf16: u32,
    pub end_utf16: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindPosition {
    pub parent_ordinal: u32,
    pub column: String,
    pub span: Option<FindSpan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FindScope {
    Field(String),
    Table,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindMatch {
    Any,
    Whole,
    Start,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindDirection {
    All,
    Up,
    Down,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindRequest {
    pub target: FindTarget,
    pub scope: FindScope,
    pub pattern: String,
    pub matching: FindMatch,
    pub direction: FindDirection,
    pub start: Option<FindPosition>,
    pub match_case: bool,
    pub formatted: bool,
    pub format_options: Options,
}

#[derive(Debug, Error)]
pub enum FindError {
    #[error("enter a Find pattern; use two quotation marks to find an empty string")]
    EmptyPattern,
    #[error("find pattern exceeds the 64 KiB input limit")]
    PatternLimit,
    #[error("find target or continuation no longer identifies a native field and record")]
    InvalidTarget,
    #[error(
        "unformatted date Find requires a regional typed-input policy; use supported formatted search"
    )]
    UnformattedDate,
    #[error(
        "unformatted fractional or currency Find requires a regional numeric policy; use supported formatted search"
    )]
    UnformattedNumber,
    #[error("the selected native field is not supported by Find")]
    UnsupportedField,
    #[error("the selected native field's format or input mask cannot be searched faithfully")]
    UnsupportedFormat,
    #[error("the database wildcard mode is unknown")]
    WildcardMode,
}
