//! Fresh sparse records use declared physical offsets and logical mask numbers.
use crate::fixtures::access::put16;
use std::borrow::Cow;
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const DEFINITION: usize = 7 * PAGE;
const FIELD_START: usize = 75;
const FIELD_BYTES: usize = 25;
const HEADER_BYTES: usize = 2;

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn text(value: &str, compressed: bool) -> Vec<u8> {
    let units: Vec<_> = value.encode_utf16().collect();
    if compressed && units.len() > 2 && units.iter().all(|unit| (1..=255).contains(unit)) {
        let mut bytes = vec![255, 254];
        bytes.extend(units.iter().map(|unit| *unit as u8));
        bytes
    } else {
        assert!(!value.starts_with('\u{feff}'));
        units.into_iter().flat_map(u16::to_le_bytes).collect()
    }
}

pub(super) fn build(before: &[u8], id: i32, values: &[(usize, Value)]) -> Vec<u8> {
    let fields = word(before, DEFINITION + 41);
    let variables = word(before, DEFINITION + 43);
    let active = word(before, DEFINITION + 45);
    assert_eq!(values.len() + 1, active);
    let mut fixed_end = HEADER_BYTES;
    for index in 0..active {
        let at = DEFINITION + FIELD_START + index * FIELD_BYTES;
        if before[at + 15] & 1 != 0 {
            assert_eq!(before[at], 4);
            fixed_end = fixed_end.max(HEADER_BYTES + word(before, at + 21) + 4);
        }
    }
    let mut bytes = vec![0; fixed_end];
    put16(&mut bytes, 0, fields as u16);
    let mut mask = vec![0; fields.div_ceil(8)];
    let mut payloads: Vec<Option<Cow<'_, [u8]>>> = (0..variables).map(|_| None).collect();
    let generated = Value::Int32(id);
    for index in 0..active {
        let at = DEFINITION + FIELD_START + index * FIELD_BYTES;
        let number = word(before, at + 5);
        let value = if number == 0 {
            assert_eq!(before[at + 15], 7);
            &generated
        } else {
            let mut found = values.iter().filter(|(native, _)| *native == number);
            let value = &found.next().unwrap().1;
            assert!(found.next().is_none());
            value
        };
        if !matches!(value, Value::Null) {
            mask[number / 8] |= 1 << (number % 8);
        }
        if before[at + 15] & 1 != 0 {
            let start = HEADER_BYTES + word(before, at + 21);
            match value {
                Value::Int32(value) => {
                    bytes[start..start + 4].copy_from_slice(&value.to_le_bytes())
                }
                Value::Null => {}
                _ => panic!("expected LONG outcome"),
            }
        } else {
            let payload = match (before[at], value) {
                (_, Value::Null) => Cow::Borrowed(&[][..]),
                (10, Value::Text(value)) => {
                    assert!(value.encode_utf16().count() <= word(before, at + 23) / 2);
                    Cow::Owned(text(value, before[at + 16] & 1 != 0))
                }
                (9, Value::Binary(value)) => {
                    assert!(value.len() <= word(before, at + 23));
                    Cow::Borrowed(value.as_slice())
                }
                _ => panic!("expected Text/Binary outcome"),
            };
            let slot = word(before, at + 7);
            assert!(payloads[slot].replace(payload).is_none());
        }
    }
    if variables != 0 {
        let mut offsets = Vec::new();
        for payload in payloads {
            offsets.push(bytes.len());
            if let Some(payload) = payload {
                bytes.extend_from_slice(&payload);
            }
        }
        offsets.push(bytes.len());
        for offset in offsets.into_iter().rev() {
            bytes.extend((offset as u16).to_le_bytes());
        }
        bytes.extend((variables as u16).to_le_bytes());
    }
    bytes.extend(mask);
    bytes
}
