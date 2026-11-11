//! The raw original demonstrates a real logical-count collision without SQL replay.
use super::source;

fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}
fn logical_start(bytes: &[u8]) -> usize {
    let physical = long(bytes, 51) as usize;
    let fields = 63 + physical * 12;
    let active = word(bytes, 45);
    let mut names = fields + active * 25;
    for _ in 0..active {
        names += 2 + word(bytes, names);
    }
    names + physical * 52
}

#[test]
fn original_count_is_occupied_and_new_long_null_index_uses_independent_identity_three() {
    let case = source();
    let base = case.target_definition as usize * 4096;
    let old = &case.before[base..base + 4096];
    let new = &case.expected[base..base + 4096];
    let at = logical_start(old);
    assert_eq!(long(old, 47), 2);
    assert_eq!([long(old, at + 4), long(old, at + 32)], [0, 2]);
    let at = logical_start(new);
    assert_eq!(long(new, 47), 3);
    assert_eq!(long(new, 51), 3);
    assert_eq!(
        [long(new, at + 4), long(new, at + 32), long(new, at + 60)],
        [0, 2, 3]
    );
    assert_eq!(long(new, at + 64), 2);
    assert_eq!(case.column, "OtherScore");
    assert_eq!(case.index_name, "OtherLookup");
    assert_eq!(case.rows.len(), 3);
    assert!(case.rows.iter().all(|row| row.score.is_none()));
    assert_eq!(case.index_pages.len(), 1);
    assert_eq!(case.root as usize, case.before.len() / 4096);
    assert_eq!(case.expected.len(), case.before.len() + 2 * 4096);
    for page in 0..case.before.len() / 4096 {
        if page != 1 && page != case.target_definition as usize {
            assert!(
                case.before[page * 4096..(page + 1) * 4096]
                    == case.expected[page * 4096..(page + 1) * 4096],
                "protectedpage {page}"
            );
        }
    }
    assert_eq!(
        &old[16..47],
        &new[16..47],
        "no rows/generator/fieldmetadata synthesized"
    );
}
