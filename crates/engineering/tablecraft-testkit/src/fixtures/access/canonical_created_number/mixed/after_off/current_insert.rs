//! Literal current sparse INSERT outcomes, independent of native serializers.

use super::deleted_field::populated::{self as dropped, Case, readdition::Target};
use crate::fixtures::access::{
    generated_record_insert::{
        placement::{self, Pages},
        record,
    },
    saved_field_rename::fitting::current_history,
};
use tablecraft_core::values::Value;

#[derive(Clone, Copy, Debug)]
pub enum Input {
    Omitted,
    Null,
    Number123,
    Empty,
    Compressed,
    Unicode,
}

pub const NUMBER_INPUTS: [Input; 3] = [Input::Omitted, Input::Null, Input::Number123];
pub const TEXT_INPUTS: [Input; 5] = [
    Input::Omitted,
    Input::Null,
    Input::Empty,
    Input::Compressed,
    Input::Unicode,
];

pub const fn name(target: Target, renamed: bool) -> &'static str {
    if renamed {
        "CurrentField"
    } else {
        target.name()
    }
}

pub fn source(case: Case, target: Target, filled: bool, renamed: bool) -> Vec<u8> {
    let before = current_history::source(case, target, filled);
    if renamed {
        current_history::rename(&before, target.name(), name(target, true))
    } else {
        before
    }
}

// These are literal resolved outcomes, not a property/default interpreter.
// Omitted Number declarations in this family have stored default zero; its
// nullable Text and Binary declarations have no default and resolve to NULL.
fn fields(case: Case, target: Target, input: Input) -> Vec<(usize, Value)> {
    let mut values = match case {
        Case::MixedText => vec![(1, Value::Null), (2, Value::Int32(0))],
        Case::MixedBinary => vec![(1, Value::Null), (2, Value::Int32(0)), (3, Value::Null)],
        Case::IdOnlyText => Vec::new(),
        _ => vec![(1, Value::Int32(0))],
    };
    let selected = match (target, input) {
        (Target::Number, Input::Omitted) => Value::Int32(0),
        (_, Input::Null) | (Target::ShortText, Input::Omitted) => Value::Null,
        (Target::Number, Input::Number123) => Value::Int32(123),
        (Target::ShortText, Input::Empty) => Value::Text(String::new()),
        (Target::ShortText, Input::Compressed) => Value::Text("new label".into()),
        (Target::ShortText, Input::Unicode) => Value::Text("日本語🦀".into()),
        _ => panic!("input does not belong to this declaration"),
    };
    values.push((
        usize::from(dropped::identity(case).native_number) + 1,
        selected,
    ));
    values
}

pub fn append(before: &[u8], case: Case, target: Target, input: Input) -> Vec<u8> {
    append_row(before, &fields(case, target, input))
}

pub fn append_with_binary(
    before: &[u8],
    case: Case,
    target: Target,
    input: Input,
    binary: &[u8],
) -> Vec<u8> {
    assert!(matches!(case, Case::MixedBinary));
    let mut values = fields(case, target, input);
    values
        .iter_mut()
        .find(|(number, _)| *number == 3)
        .unwrap()
        .1 = Value::Binary(binary.to_vec());
    append_row(before, &values)
}

/// Values are keyed by live native number; the generated ID is authored from
/// the original LAST counter, never supplied by a caller or inferred from slots.
pub fn append_row(before: &[u8], values: &[(usize, Value)]) -> Vec<u8> {
    assert_eq!(before.len(), 11 * 4096);
    let pages = Pages {
        definition: 7,
        data: 8,
        index: 9,
    };
    let id = placement::next_id(before, pages);
    let row = record::build(before, pages.definition, id, values);
    placement::append(before, pages, id, &row)
}

pub fn row(case: Case, target: Target, input: Input, id: i32) -> Vec<Value> {
    let mut values = vec![Value::Int32(id)];
    values.extend(
        fields(case, target, input)
            .into_iter()
            .map(|(_, value)| value),
    );
    values
}

pub fn row_with_binary(
    case: Case,
    target: Target,
    input: Input,
    id: i32,
    binary: &[u8],
) -> Vec<Value> {
    assert!(matches!(case, Case::MixedBinary));
    let mut values = row(case, target, input, id);
    values[3] = Value::Binary(binary.to_vec());
    values
}
