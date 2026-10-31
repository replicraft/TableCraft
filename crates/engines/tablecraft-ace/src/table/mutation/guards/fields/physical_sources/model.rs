//! Independent complete images from original source plus literal macro semantics.
use crate::preservation::table_extra::tests::guard_sources::model as programs;
use crate::table::mutation::fixed::batch::split_tests::model as original;
use tablecraft_testkit::access as raw;
use tablecraft_testkit::access::multiple_indexes as frames;

pub(super) const PAGE: usize = 4096;
pub(super) const TARGET: usize = 300;
pub(super) const BODY: &str = "<Statements><Action Name=\"SetField\"><Argument Name=\"Field\">Group</Argument><Argument Name=\"Value\">[Score]</Argument></Action><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[ID]</Argument></Action></Statements>";
pub(super) const LATE_BODY: &str = "<Statements><Action Name=\"SetField\"><Argument Name=\"Field\">Group</Argument><Argument Name=\"Value\">[Score]</Argument></Action><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[ID]</Argument></Action><ConditionalBlock><If><Condition>[ID]=302</Condition><Statements><Action Name=\"RaiseError\"><Argument Name=\"Number\">17</Argument><Argument Name=\"Description\">rejected</Argument></Action></Statements></If></ConditionalBlock></Statements>";

pub(super) struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub original: Vec<original::Row>,
    pub final_rows: Vec<original::Row>,
    pub xml: String,
    pub extra: Vec<u8>,
    pub changed: bool,
}

fn catalog_record(bytes: &[u8], slot: usize) -> &[u8] {
    let page = &bytes[4 * PAGE..5 * PAGE];
    assert_eq!(u16::from_le_bytes(page[12..14].try_into().unwrap()), 3);
    let directory = 14 + slot * 2;
    let start = usize::from(u16::from_le_bytes(
        page[directory..directory + 2].try_into().unwrap(),
    ));
    let end = if slot == 0 {
        PAGE
    } else {
        usize::from(u16::from_le_bytes(
            page[directory - 2..directory].try_into().unwrap(),
        ))
    };
    assert_eq!(start & 0xe000, 0);
    &page[start..end]
}

fn component(value: Option<i32>, descending: bool) -> Vec<u8> {
    let mut bytes = match value {
        None => vec![0],
        Some(value) => {
            let mut bytes = vec![127];
            bytes.extend(((value as u32) ^ 0x8000_0000).to_be_bytes());
            bytes
        }
    };
    if descending {
        for byte in &mut bytes {
            *byte = !*byte;
        }
    }
    bytes
}

pub(super) fn entries(rows: &[original::Row], physical: usize, descending: bool) -> Vec<Vec<u8>> {
    let mut keys = Vec::new();
    for row in rows {
        if physical == 1 && row.left.is_none() && row.right.is_none() {
            continue;
        }
        let mut wire = if physical == 0 {
            component(row.left, descending)
        } else {
            let mut wire = component(row.right, !descending);
            wire.extend(component(row.left, descending));
            wire
        };
        let key_length = wire.len();
        wire.extend(row.reference.to_be_bytes());
        keys.push((wire, key_length, row.reference));
    }
    keys.sort_by(|left, right| (&left.0[..left.1], left.2).cmp(&(&right.0[..right.1], right.2)));
    keys.into_iter().map(|entry| entry.0).collect()
}

pub(super) fn source(code: u8, descending: bool, unique: bool, late_raise: bool) -> Case {
    let original_case = original::source(code, original::Shape::Mixed, descending, unique, false);
    let mut before = original_case.before;
    let body = if late_raise { LATE_BODY } else { BODY };
    let (program_image, xml, extra) = programs::source(code, "BeforeChange", body);
    let records = [
        (0, catalog_record(&before, 0).to_vec()),
        (0, catalog_record(&before, 1).to_vec()),
        (0, catalog_record(&program_image, 2).to_vec()),
    ];
    before[4 * PAGE..5 * PAGE].copy_from_slice(&raw::data_page(2, &records));
    let rows = original_case.original;
    assert_eq!(rows[TARGET].id, 301);
    assert!(rows[TARGET].left.is_some());
    assert!(rows[TARGET].right.is_none());

    let mut final_rows = rows.clone();
    let mut expected = before.clone();
    let mut changed_record = None;
    if !late_raise {
        // Caller requests Score=NULL->99; SetField reads that proposed Current,
        // writes Group=99, then reads original ID=301 into Score. Both components
        // of PairOrder change together; aliases share those two physical owners.
        final_rows[TARGET].left = Some(99);
        final_rows[TARGET].right = Some(301);
        let reference = rows[TARGET].reference;
        let page = (reference >> 8) as usize;
        let slot = (reference & 255) as usize;
        let directory = page * PAGE + 14 + slot * 2;
        let start = page * PAGE
            + usize::from(u16::from_le_bytes(
                before[directory..directory + 2].try_into().unwrap(),
            ));
        let end = if slot == 0 {
            (page + 1) * PAGE
        } else {
            page * PAGE
                + usize::from(u16::from_le_bytes(
                    before[directory - 2..directory].try_into().unwrap(),
                ))
        };
        assert_eq!(end - start, 15);
        changed_record = Some(start + 6..end);
        assert_eq!(&before[start..start + 2], &3_u16.to_le_bytes());
        assert_eq!(&before[start + 2..start + 6], &301_i32.to_le_bytes());
        assert_eq!(before[start + 14], 3);
        expected[start + 6..start + 10].copy_from_slice(&99_i32.to_le_bytes());
        expected[start + 10..start + 14].copy_from_slice(&301_i32.to_le_bytes());
        expected[start + 14] = 7;

        for physical in 0..2 {
            let page = 13 + physical;
            let start = page * PAGE;
            let old = &before[start..start + PAGE];
            assert_eq!(&old[..2], &[4, 1]);
            assert_eq!(&old[24..26], &[0, 0]);
            let keys = entries(&final_rows, physical, descending);
            let leaf = frames::leaf(Some(old), &keys, 0, 0, 0);
            expected[start..start + PAGE].copy_from_slice(&leaf);
        }
    }
    if let Some(record) = changed_record {
        let mut cursor = 0;
        // The only allowed locations are both complete independently rendered
        // INDEX leaves and the two fixed cells plus original row's mask byte.
        for span in [13 * PAGE..15 * PAGE, record] {
            assert!(cursor <= span.start);
            assert_eq!(&before[cursor..span.start], &expected[cursor..span.start]);
            cursor = span.end;
        }
        assert_eq!(&before[cursor..], &expected[cursor..]);
    } else {
        assert_eq!(before, expected);
    }
    Case {
        before,
        expected,
        original: rows,
        final_rows,
        xml,
        extra,
        changed: !late_raise,
    }
}
