use super::{FIELD_BYTES, FIELD_START, PAGE_BYTES, TEMPLATE_PAGE, database, read};
use tablecraft_ace::InspectError;

const OWNER_PAGE: usize = 10;
const FLAT_PAGE: usize = 20;
const MEMO_POSITION: usize = 2;
const FLAGS: usize = 15;
const EXTENDED_FLAGS: usize = 16;

fn visible(flags: u8, extended: u8) -> Vec<u8> {
    let mut bytes = database();
    let column = OWNER_PAGE * PAGE_BYTES + FIELD_START + MEMO_POSITION * FIELD_BYTES;
    bytes[column + FLAGS] = flags;
    bytes[column + EXTENDED_FLAGS] = extended;
    bytes
}

#[test]
fn ordinary_visible_memo_flag_family_preserves_current_codec_and_history_association() {
    for flags in [0x02, 0x82] {
        for extended in [0, 1] {
            let bytes = visible(flags, extended);
            let history = read(&bytes, "Notes").unwrap();
            assert_eq!(history.memo_column().flags(), flags);
            assert_eq!(history.memo_column().extended_flags(), extended);
            assert_eq!(history.links().column().number(), 1);
            let versions = history
                .stored_versions(0)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(versions[0].value(), Some("é🦀\0older"));
            assert_eq!(versions[1].value(), Some("newer"));
        }
    }
}

#[test]
fn hyperlink_marker_does_not_admit_calculation_fixed_unknown_or_child_flags() {
    for flags in [0x80, 0x83, 0x86, 0x8a, 0xff] {
        assert_eq!(
            read(&visible(flags, 1), "Notes").err(),
            Some(InspectError::Unsupported)
        );
    }
    for extended in [2, 3, 0x10, 0x11, 0xc0, 0xc1] {
        assert_eq!(
            read(&visible(0x82, extended), "Notes").err(),
            Some(InspectError::Unsupported)
        );
    }
    for (page, position) in [(TEMPLATE_PAGE, 0), (FLAT_PAGE, 1)] {
        let mut bytes = visible(0x82, 1);
        bytes[page * PAGE_BYTES + FIELD_START + position * FIELD_BYTES + FLAGS] = 0x82;
        assert_eq!(read(&bytes, "Notes").err(), Some(InspectError::Unsupported));
    }
}
