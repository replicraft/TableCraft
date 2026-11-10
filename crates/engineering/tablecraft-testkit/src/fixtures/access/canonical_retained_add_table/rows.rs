//! Literal generated row outcomes in fresh retained-ledger tables.
use crate::fixtures::access::generated_record_insert::{
    placement::{self, Pages},
    record,
};
use tablecraft_core::values::Value;

#[derive(Clone, Copy, Debug)]
pub enum Profile {
    NativeNumber,
    NativeText,
    DesktopNumber,
    DesktopText,
}

#[derive(Clone, Copy, Debug)]
pub enum Input<'a> {
    Omitted,
    Null,
    Number(i32),
    Text(&'a str),
}

fn resolved(profile: Profile, input: Input<'_>) -> Value {
    match (profile, input) {
        (Profile::DesktopNumber, Input::Omitted) => Value::Int32(0),
        (_, Input::Omitted | Input::Null) => Value::Null,
        (Profile::NativeNumber | Profile::DesktopNumber, Input::Number(number)) => {
            Value::Int32(number)
        }
        (Profile::NativeText | Profile::DesktopText, Input::Text(text)) => {
            assert!(!text.is_empty() || matches!(profile, Profile::DesktopText));
            Value::Text(text.into())
        }
        _ => panic!("input does not belong to this literal declaration"),
    }
}

/// Exact fresh scalar carriers, independent of property decoding or native defaults.
pub fn row(profile: Profile, input: Input<'_>, id: i32) -> Vec<Value> {
    vec![Value::Int32(id), resolved(profile, input)]
}

/// Native counters generate the ID; physical DATA frontier generates its ROW key.
/// Existing sibling, tombstone, property and allocation pages remain unchanged.
pub fn append(before: &[u8], definition: u32, profile: Profile, input: Input<'_>) -> Vec<u8> {
    let definition = usize::try_from(definition).unwrap();
    let pages = Pages {
        definition,
        data: definition + 1,
        index: definition + 2,
    };
    let start = definition * 4096;
    assert_eq!(
        u16::from_le_bytes(before[start + 41..start + 43].try_into().unwrap()),
        2
    );
    assert_eq!(
        u16::from_le_bytes(before[start + 45..start + 47].try_into().unwrap()),
        2
    );
    assert_eq!(before[start + 75 + 15], 7);
    assert_eq!(
        before[start + 100],
        if matches!(profile, Profile::NativeNumber | Profile::DesktopNumber) {
            4
        } else {
            10
        }
    );
    let id = placement::next_id(before, pages);
    let values = [(1, resolved(profile, input))];
    let row = record::build(before, definition, id, &values);
    placement::append(before, pages, id, &row)
}
