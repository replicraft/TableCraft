use super::{Payload, encode};
use crate::EditError;
use tablecraft_core::values::{Guid, Value};
use tablecraft_schema::ScalarType;

#[test]
fn fixed_payload_specialization_keeps_original_eight_byte_allocation_geometry() {
    let alignment = align_of::<usize>();
    let old_bytes = (2 * size_of::<usize>() + 16).next_multiple_of(alignment);
    let guid_bytes = (2 * size_of::<usize>() + 32).next_multiple_of(alignment);
    assert_eq!(size_of::<Payload<8>>(), old_bytes);
    assert_eq!(size_of::<Payload<16>>(), guid_bytes);
    assert_eq!(
        encode::<8>(ScalarType::Int16, &Value::Int16(-32768)).unwrap(),
        Some([0, 128, 0, 0, 0, 0, 0, 0])
    );
}

#[test]
fn guid_stage_uses_native_word_order_only_in_the_sixteen_byte_specialization() {
    let value = Value::Guid(Guid::from_bytes([
        0x6b, 0x29, 0xfc, 0x40, 0xca, 0x47, 0x10, 0x67, 0xb3, 0x1d, 0, 0xdd, 1, 6, 0x62, 0xda,
    ]));
    assert_eq!(
        encode::<16>(ScalarType::Guid, &value).unwrap(),
        Some([
            0x40, 0xfc, 0x29, 0x6b, 0x47, 0xca, 0x67, 0x10, 0xb3, 0x1d, 0, 0xdd, 1, 6, 0x62, 0xda,
        ])
    );
    assert_eq!(
        encode::<8>(ScalarType::Guid, &value),
        Err(EditError::Preservation)
    );
    assert_eq!(encode::<16>(ScalarType::Guid, &Value::Null).unwrap(), None);
}
