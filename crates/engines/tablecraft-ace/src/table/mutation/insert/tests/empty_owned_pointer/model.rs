//! Original empty membership and a first INSERT image, without producer codecs.
use tablecraft_core::values::Value;
use tablecraft_testkit::access as literal;
use tablecraft_testkit::access::insert_seed as original;
use tablecraft_testkit::access::multiple_indexes as frames;

pub const PAGE: usize = 4096;
pub const DATA: u32 = 16;
pub const BITMAP: u32 = 17;

pub struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub values: Vec<Value>,
}

pub fn case(code: u8) -> Case {
    let mut before = original::source(code, true, false, None);
    literal::put32(&mut before, 10 * PAGE + 16, 0);
    literal::put32(&mut before, 10 * PAGE + 67, 0);
    let mut indirect = vec![0; 69];
    indirect[0] = 1;
    frames::map_page(&mut before, 11, vec![indirect.clone(), frames::map([])]);
    literal::put16(&mut before, 11 * PAGE + 2, 3940);
    let empty = frames::leaf(Some(&before[13 * PAGE..14 * PAGE]), &[], 0, 0, 0);
    frames::install(&mut before, 13, &empty);
    assert_eq!(before.len(), 16 * PAGE);
    // DATA12 remains physically occupied and globally allocated, but it is not
    // owned by this empty table and must never be adopted as a destination.
    assert_eq!(
        u16::from_le_bytes(before[12 * PAGE + 12..12 * PAGE + 14].try_into().unwrap()),
        3
    );

    let mut expected = before.clone();
    expected.resize(18 * PAGE, 0);
    let record = vec![2, 0, 7, 0, 0, 0, 0x80, 0, 0xff, 9, 0, 6, 0, 1, 0, 3];
    let mut data = literal::data_page(10, &[(0, record)]);
    literal::put16(&mut data, 2, 4064);
    frames::install(&mut expected, DATA, &data);
    let mut bitmap = vec![0; PAGE];
    bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
    bitmap[6] = 1;
    frames::install(&mut expected, BITMAP, &bitmap);
    indirect[1..5].copy_from_slice(&BITMAP.to_le_bytes());
    frames::map_page(&mut expected, 11, vec![indirect, frames::map([DATA])]);
    literal::put16(&mut expected, 11 * PAGE + 2, 3940);
    frames::map_page(&mut expected, 1, vec![frames::map(18..512)]);
    let key = vec![127, 128, 0, 0, 7, 0, 0, 16, 0];
    let root = frames::leaf(Some(&before[13 * PAGE..14 * PAGE]), &[key], 0, 0, 0);
    frames::install(&mut expected, 13, &root);
    literal::put32(&mut expected, 10 * PAGE + 16, 1);
    literal::put32(&mut expected, 10 * PAGE + 67, 1);
    assert_eq!(
        &expected[12 * PAGE..13 * PAGE],
        &before[12 * PAGE..13 * PAGE]
    );
    Case {
        before,
        expected,
        values: vec![Value::Int32(7), Value::Binary(vec![0x80, 0, 0xff])],
    }
}
