//! Source-literal witnesses and TCND bytes; no native mutation implementation.
use sha2::{Digest, Sha256};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_binary_delete as sources;

pub use sources::{Case, Recipe};

pub fn record(slot: usize) -> Vec<u8> {
    let values = sources::original_rows();
    let Value::Int32(identity) = &values[slot][0] else {
        panic!("frozen manual Id");
    };
    let Value::Int32(marker) = &values[slot][2] else {
        panic!("frozen manual Marker");
    };
    let payload = match &values[slot][1] {
        Value::Null => None,
        Value::Binary(bytes) => Some(bytes.as_slice()),
        _ => panic!("frozen nullable Binary"),
    };
    let mut result = Vec::from(3_u16.to_le_bytes());
    result.extend_from_slice(&identity.to_le_bytes());
    result.extend_from_slice(&marker.to_le_bytes());
    let length = payload.map_or(0, <[u8]>::len);
    if let Some(bytes) = payload {
        result.extend_from_slice(bytes);
    }
    for word in [10 + length as u16, 10, 1] {
        result.extend_from_slice(&word.to_le_bytes());
    }
    result.push(if payload.is_some() { 7 } else { 5 });
    assert_eq!(result.len(), 17 + length);
    result
}

pub fn cases() -> Vec<Case> {
    let cases = sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    });
    // Check the literal raw witnesses against every original before returning
    // any case to a caller that can invoke a matching producer.
    for case in &cases {
        let data = case.data as usize * 4096;
        let mut end = 4096;
        for slot in 0..13 {
            let at = data + 14 + slot * 2;
            let start = usize::from(u16::from_le_bytes(
                case.before[at..at + 2].try_into().unwrap(),
            ));
            assert!(start <= end && start < 4096);
            let expected = record(slot);
            assert_eq!(&case.before[data + start..data + end], expected.as_slice());
            end = start;
        }
        assert_eq!(end, 3816);
    }
    cases
}

pub fn witnesses(case: &Case, reverse: bool) -> Vec<(usize, Vec<u8>)> {
    let mut result = case
        .recipe
        .selected()
        .iter()
        .map(|slot| (*slot, record(*slot)))
        .collect::<Vec<_>>();
    if reverse {
        result.reverse();
    }
    result
}

pub fn forged_offsets(case: &Case) -> Vec<usize> {
    let td = case.definition as usize * 4096;
    let data = case.data as usize * 4096;
    let first = case.recipe.selected()[0];
    vec![
        td + 16,
        td + 67,
        td + 79,
        (case.definition as usize + 1) * 4096 + 4095,
        data + 2,
        data + 15 + 2 * first,
        data + 4095,
        case.roots[0] as usize * 4096 + 480 + 8,
        case.roots[1] as usize * 4096 + 27,
        case.roots[2] as usize * 4096 + 4095,
    ]
}
