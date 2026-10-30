//! Independent additive large-file source/expected pair, before transport runtime.
use super::{cases, original};
use tablecraft_core::values::Value;

pub(super) fn case() -> cases::Case {
    let before = original::null_memo_pressure_source();
    let mut expected = before.clone();
    // Literal engineering page framing: 262144 payload bytes occupy 64 full
    // 4072-byte fragments and one 1536-byte fragment, followed by the parent.
    let sizes = std::iter::repeat_n(4072, 64).chain([1536]);
    expected.resize(90 * 4096, 0);
    for (position, size) in sizes.enumerate() {
        let number = 24 + position;
        let next = if position == 64 {
            0
        } else {
            ((number + 1) << 8) as u32
        };
        let mut chunk = next.to_le_bytes().to_vec();
        chunk.extend([0, 1].repeat(size / 2));
        original::source::install(
            &mut expected,
            number,
            &original::source::lval(&[(0, chunk)]),
        );
        original::set_member(&mut expected, 2, number, true);
    }
    original::set_member(&mut expected, 3, 13, false);
    original::set_member(&mut expected, 3, 88, true);
    original::set_member(&mut expected, 0, 89, true);
    original::set_member(&mut expected, 1, 12, false);
    original::set_member(&mut expected, 1, 89, true);

    let descriptor = original::source::descriptor(0, 262144, 24 << 8);
    let ole = original::source::single(1, 14, 0);
    let parent = original::source::record(1, Some(&descriptor), Some(&ole));
    original::source::install(
        &mut expected,
        89,
        &original::source::page(10, &[(0x8000, parent)]),
    );
    let start = 12 * 4096 + usize::from(original::word(&before, 12, 0) & 0x1fff);
    let root_length = original::body(&before, 12, 0).len();
    expected[start..start + root_length].fill(0);
    expected[start..start + 4].copy_from_slice(&(89_u32 << 8).to_le_bytes());
    expected[12 * 4096 + 14..12 * 4096 + 16]
        .copy_from_slice(&(original::word(&before, 12, 0) | 0x4000).to_le_bytes());

    let global_start = usize::from(original::word(&before, 1, 0) & 0x1fff);
    for number in 24..90 {
        expected[4096 + global_start + 5 + number / 8] &= !(1 << (number % 8));
    }

    cases::Case {
        before,
        expected,
        column: "Memo",
        before_value: Value::Null,
        value: Value::Text("Ā".repeat(131072)),
    }
}
