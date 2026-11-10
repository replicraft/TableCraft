//! Informational common-filter metadata and independently checked typed requests.
mod service;

use crate::scalar_input::InputHint;
use tablecraft_schema::ScalarType;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldFilterTarget {
    pub document_revision: u64,
    pub table: String,
    pub column: String,
    pub filter_id: Option<u64>,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldFilterOperator {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Between,
    NotBetween,
    IsNull,
    IsNotNull,
    BeginsWith,
    NotBeginsWith,
    EndsWith,
    NotEndsWith,
    Contains,
    NotContains,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldFilterPatternMode {
    Ansi89,
    Ansi92,
}

pub struct FieldFilterDescription {
    pub target: FieldFilterTarget,
    pub input_hint: InputHint,
    /// Reader nullability, not native Required or a permission to write NULL.
    pub nullable: bool,
    pub operators: Vec<FieldFilterOperator>,
    pub pattern_mode: Option<FieldFilterPatternMode>,
}

const ORDERED_OPERATORS: &[FieldFilterOperator] = &[
    FieldFilterOperator::Equal,
    FieldFilterOperator::NotEqual,
    FieldFilterOperator::Less,
    FieldFilterOperator::LessOrEqual,
    FieldFilterOperator::Greater,
    FieldFilterOperator::GreaterOrEqual,
    FieldFilterOperator::Between,
    FieldFilterOperator::NotBetween,
    FieldFilterOperator::IsNull,
    FieldFilterOperator::IsNotNull,
];
const TEXT_OPERATORS: &[FieldFilterOperator] = &[
    FieldFilterOperator::Equal,
    FieldFilterOperator::NotEqual,
    FieldFilterOperator::BeginsWith,
    FieldFilterOperator::NotBeginsWith,
    FieldFilterOperator::EndsWith,
    FieldFilterOperator::NotEndsWith,
    FieldFilterOperator::Contains,
    FieldFilterOperator::NotContains,
    FieldFilterOperator::IsNull,
    FieldFilterOperator::IsNotNull,
];

pub(super) fn operators(kind: ScalarType) -> Option<&'static [FieldFilterOperator]> {
    match kind {
        ScalarType::Text => Some(TEXT_OPERATORS),
        ScalarType::Boolean | ScalarType::Binary | ScalarType::Guid => None,
        _ => Some(ORDERED_OPERATORS),
    }
}

impl FieldFilterOperator {
    pub fn input_count(self) -> usize {
        match self {
            Self::IsNull | Self::IsNotNull => 0,
            Self::Between | Self::NotBetween => 2,
            _ => 1,
        }
    }

    pub(super) fn negated_pattern(self) -> bool {
        matches!(
            self,
            Self::NotEqual | Self::NotBeginsWith | Self::NotEndsWith | Self::NotContains
        )
    }
}
