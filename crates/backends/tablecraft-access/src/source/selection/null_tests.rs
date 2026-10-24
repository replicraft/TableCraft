use super::{Selection, selection};
use std::ops::Bound::Included;
use tablecraft_ace::{BinaryIndexSelection, TextIndexSelection};
use tablecraft_query::source::ScanConstraint;
use tablecraft_schema::ScalarType;

#[test]
fn null_hints_select_typed_text_and_binary_paths_without_string_bounds() {
    assert!(matches!(
        selection(ScanConstraint::Null { column: 1 }, ScalarType::Text),
        Some(Selection::Text(TextIndexSelection::Null))
    ));
    assert!(matches!(
        selection(ScanConstraint::Null { column: 1 }, ScalarType::Binary),
        Some(Selection::Binary(BinaryIndexSelection::Null))
    ));
}

#[test]
fn numeric_bounds_do_not_become_text_or_binary_intervals() {
    let constraint = ScanConstraint::Int32 {
        column: 1,
        lower: Included(1),
        upper: Included(1),
    };
    for kind in [ScalarType::Text, ScalarType::Binary] {
        assert!(selection(constraint, kind).is_none());
    }
}
