//! Independent fitted image over the already qualified joint-source owner.
use super::frames;

pub(super) struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub aliased: Vec<u8>,
    pub aliased_expected: Vec<u8>,
    pub rows: Vec<frames::Row>,
    pub final_rows: Vec<frames::Row>,
}

pub(super) fn case() -> Case {
    let source = frames::case(6, frames::Family::Legacy, false, frames::Shape::Root);
    let before = source.before;
    let rows = source.rows;
    let mut final_rows = rows.clone();
    final_rows[0].age = 99;
    final_rows[0].name = Some(b"a".to_vec());
    let mut expected = before.clone();

    let first = 12 * 4096 + 14;
    let start = usize::from(u16::from_le_bytes(
        before[first..first + 2].try_into().unwrap(),
    ));
    assert_eq!(4096 - start, 19);
    let record = 12 * 4096 + start;
    assert_eq!(&before[record..record + 2], &[3, 0]);
    assert_eq!(&before[record + 10..record + 12], &[0xd3, 0x91]);
    assert_eq!(before[record + 18], 3);
    expected[record + 6..record + 10].copy_from_slice(&99_i32.to_le_bytes());
    expected[record + 10..record + 12].copy_from_slice(&[b'a', 0]);
    expected[record + 18] = 7;
    for (physical, root) in [13, 15].into_iter().enumerate() {
        let entries =
            frames::keys::entries(&final_rows, frames::Family::Legacy, false, physical == 1);
        let original = &before[root * 4096..(root + 1) * 4096];
        expected[root * 4096..(root + 1) * 4096].copy_from_slice(&frames::index::index_page(
            Some(original),
            &entries,
            (0, 0),
            0,
            false,
        ));
    }

    let map = 14 * 4096;
    let row = usize::from(u16::from_le_bytes(
        before[map + 14..map + 16].try_into().unwrap(),
    ));
    let cell = map + row + 5 + 12 / 8;
    assert_eq!(before[cell] & (1 << (12 % 8)), 0);
    let mut aliased = before.clone();
    let mut aliased_expected = expected.clone();
    aliased[cell] |= 1 << (12 % 8);
    aliased_expected[cell] |= 1 << (12 % 8);
    assert_eq!(
        before.iter().zip(&aliased).filter(|(a, b)| a != b).count(),
        1
    );

    Case {
        before,
        expected,
        aliased,
        aliased_expected,
        rows,
        final_rows,
    }
}
