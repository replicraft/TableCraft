//! Raw one-character text consumes its entire genuine physical field capacity.
use super::{model, qualify};
use model::{Case, Family, PAGE, Recipe};
use tablecraft_testkit::access as raw;

pub(in crate::table::mutation) fn case(family: Family, descending: bool) -> Case {
    // The independently authored raw UTF16 frame already describes the exact
    // joint permutation. The new source has declared compression but no slack.
    let mut case = model::case(6, family, descending, Recipe::Permutation, false);
    case.before[20] = 5;
    case.expected[20] = 5;
    // Three 12-byte physical counter records precede the column descriptors.
    let extended_flags = 10 * PAGE + 99 + 50 + 16;
    assert_eq!(case.before[extended_flags], 0);
    assert_eq!(case.expected[extended_flags], 0);
    case.before[extended_flags] = 1;
    case.expected[extended_flags] = 1;

    for record in case.records.iter_mut().chain(&mut case.final_records) {
        let footer = record.len() - 9;
        assert_eq!(
            &record[footer - 8..footer],
            &[0x9d, 0xe1, 0xa7, 0, 0x8f, 0xb2, 0x43, 0xc9],
        );
        record.drain(footer - 8..footer);
        let footer = record.len() - 9;
        assert_eq!(
            u16::from_le_bytes(record[footer..footer + 2].try_into().unwrap()),
            footer as u16
        );
        assert_eq!(record.len(), 25);
    }

    for (image, records) in [
        (&mut case.before, &case.records),
        (&mut case.expected, &case.final_records),
    ] {
        let rows = records
            .iter()
            .cloned()
            .map(|record| (0, record))
            .collect::<Vec<_>>();
        let mut page = raw::data_page(10, &rows);
        let free = PAGE - 14 - rows.len() * 2 - records.iter().map(Vec::len).sum::<usize>();
        raw::put16(&mut page, 2, free as u16);
        image[12 * PAGE..13 * PAGE].copy_from_slice(&page);
    }

    case
}

#[test]
fn qualify_mixed_zero_slack_raw_sources_and_complete_expected_before_preview() {
    for family in [Family::Legacy, Family::Modern] {
        for descending in [false, true] {
            let case = case(family, descending);
            qualify(&case.before, &case.rows, family);
            qualify(&case.expected, &case.final_rows, family);
            super::preservation(&case);
            assert_eq!(case.before.len(), case.expected.len());
            assert_eq!(
                &case.before[10 * PAGE..11 * PAGE],
                &case.expected[10 * PAGE..11 * PAGE]
            );
        }
    }
}
