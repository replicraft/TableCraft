//! Additive stable-root expectation; the failed compacting expectation stays intact.
use super::{cases, original};
use tablecraft_core::values::Value;

pub(super) fn case() -> cases::Case {
    let before = original::source::source(6, 1);
    let mut expected = before.clone();
    let memo = original::source::single(2, 13, 0);
    let ole = original::inline(&[8]);
    let final_record = original::source::record(1, Some(&memo), Some(&ole));
    assert_eq!(original::body(&before, 12, 0).len(), 39);
    assert_eq!(final_record.len(), 40);

    // The stable root retains its original 39 bytes. Growth appends a hidden
    // record below that root, rather than compacting/repacking the root row.
    let page = &mut expected[12 * 4096..13 * 4096];
    page[4017..4057].copy_from_slice(&final_record);
    page[4057..4096].fill(0);
    page[4057..4061].copy_from_slice(&0x0c01_u32.to_le_bytes());
    page[2..4].copy_from_slice(&3999_u16.to_le_bytes());
    page[12..14].copy_from_slice(&2_u16.to_le_bytes());
    page[14..16].copy_from_slice(&(4057_u16 | 0x4000).to_le_bytes());
    page[16..18].copy_from_slice(&(4017_u16 | 0x8000).to_le_bytes());

    cases::Case {
        before,
        expected,
        column: "Ole",
        before_value: Value::Binary(vec![7]),
        value: Value::Binary(vec![8]),
    }
}

#[test]
fn qualify_stable_root_ole_forwarding_before_consumer_replay() {
    let case = case();
    for (bytes, value) in [
        (&case.before, &case.before_value),
        (&case.expected, &case.value),
    ] {
        assert_eq!(
            original::read(bytes),
            vec![vec![
                Value::Int32(1),
                Value::Text("A".into()),
                value.clone()
            ]]
        );
        original::check(bytes);
    }
    assert_eq!(case.expected.len(), case.before.len());
    original::old_chunks_unchanged(&case.before, &case.expected);
    assert!(case.expected[..12 * 4096] == case.before[..12 * 4096]);
    assert!(case.expected[13 * 4096..] == case.before[13 * 4096..]);
}
