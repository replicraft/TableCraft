//! The qualified bitmap ends before the first unaddressable page bit.
use super::set_member;
use crate::{
    EditError, InspectError, WriteBlocker,
    format::{allocation::test_header as header, reader::Reader},
};
use std::io::Cursor;

mod rebase;
use tablecraft_core::limits::ResourceBudget;

#[test]
fn bitmap_limit_refuses_without_touching_signature_or_wrapping_membership() {
    let mut bitmap = vec![0; 4096];
    bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
    let mut source = Cursor::new([].as_slice());
    let mut budget = ResourceBudget::new(10000, 100, 10000);
    let mut reader = Reader {
        source: &mut source,
        header: header(3),
        budget: &mut budget,
        cancelled: &|| false,
        reads: 0,
    };
    set_member(&mut bitmap, 32735, &mut reader).unwrap();
    assert_eq!(bitmap[4095], 0x80);
    let before = bitmap.clone();
    assert_eq!(
        set_member(&mut bitmap, 32736, &mut reader).err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );
    assert_eq!(bitmap, before);
    assert_eq!(
        set_member(&mut bitmap, 32735, &mut reader).err(),
        Some(EditError::Inspect(InspectError::Malformed))
    );
    assert_eq!(bitmap, before);
}
