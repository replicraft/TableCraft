use super::*;
use proptest::prelude::*;

fn bytes() -> ByteString {
    // UTF-16LE for A, U+4E2D and U+1F980. Independent of production encoders.
    ByteString::from_owned(vec![65, 0, 45, 78, 62, 216, 128, 221]).unwrap()
}

#[test]
fn consuming_slices_preserve_odd_bytes_surrogate_halves_and_storage() {
    let value = bytes();
    let pointer = value.as_bytes().as_ptr();
    let value = value.left(7).unwrap().mid(4, Some(3)).unwrap();
    assert_eq!(value.as_bytes(), [78, 62, 216]);
    assert_eq!(value.as_bytes().as_ptr(), pointer.wrapping_add(3));
    assert_eq!(bytes().right(3).unwrap().as_bytes(), [216, 128, 221]);
    assert_eq!(bytes().mid(5, Some(2)).unwrap().as_bytes(), [62, 216]);
    assert_eq!(bytes().mid(9, None).unwrap().len(), 0);
    assert!(bytes().mid(i32::MAX, Some(i32::MAX)).unwrap().is_empty());
    assert_eq!(
        bytes().right(i32::MAX).unwrap().as_bytes(),
        bytes().as_bytes()
    );
    assert!(bytes().left(-1).is_err());
    assert!(bytes().right(-1).is_err());
    assert!(bytes().mid(0, None).is_err());
    assert!(bytes().mid(100, Some(-1)).is_err());
    assert!(!format!("{:?}", bytes()).contains("65"));
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn byte_slices_match_independent_vector_ranges(
        input in prop::collection::vec(any::<u8>(), 0..200),
        start in 1i32..300,
        length in 0i32..300,
    ) {
        let offset = ((start - 1) as usize).min(input.len());
        let end = (offset + length as usize).min(input.len());
        let sliced = ByteString::from_owned(input.clone()).unwrap().mid(start, Some(length)).unwrap();
        prop_assert_eq!(sliced.as_bytes(), &input[offset..end]);
        let left = ByteString::from_owned(input.clone()).unwrap().left(length).unwrap();
        prop_assert_eq!(left.as_bytes(), &input[..(length as usize).min(input.len())]);
        let right = ByteString::from_owned(input.clone()).unwrap().right(length).unwrap();
        prop_assert_eq!(right.as_bytes(), &input[input.len().saturating_sub(length as usize)..]);
    }
}
