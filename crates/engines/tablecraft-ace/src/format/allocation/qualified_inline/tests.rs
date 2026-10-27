//! Literal native records exercise framing without granting mutation authority.
use super::QualifiedInlineRecord;
use crate::{
    InspectError,
    format::{
        allocation::{self, tests::header},
        reader::Reader,
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

fn file(width: usize, kind: u8) -> Vec<u8> {
    let mut bytes = vec![0; 3 * 4096];
    let start = 4096 - width;
    bytes[4096..4098].copy_from_slice(&[1, 1]);
    bytes[4096 + 12..4096 + 14].copy_from_slice(&1_u16.to_le_bytes());
    bytes[4096 + 14..4096 + 16].copy_from_slice(&(start as u16).to_le_bytes());
    bytes[4096 + start] = kind;

    bytes
}

#[test]
fn observed_record_preserves_before_bytes_and_serializes_the_indirect_pointer() {
    let mut before = file(69, 0);
    before[8192 - 69 + 1..8192 - 69 + 5].copy_from_slice(&128_u32.to_le_bytes());
    before[8192 - 64] = 0x81;
    before[8191] = 0x40;
    let original = before[8192 - 69..8192].to_vec();
    let mut source = Cursor::new(before);
    let mut budget = ResourceBudget::new(1_000_000, 100, 1_000_000);
    let mut reader = Reader {
        source: &mut source,
        header: header(3),
        budget: &mut budget,
        cancelled: &|| false,
        reads: 0,
    };
    let record = QualifiedInlineRecord::capture(&mut reader, 1 << 8).unwrap();
    assert_eq!(record.reference(), 1 << 8);
    assert_eq!(record.range(), 8123..8192);
    assert_eq!(record.before(), original);
    assert_eq!(record.start().unwrap(), 128);
    let after = record.indirect(2, &mut reader).unwrap();
    assert_eq!(&after[..5], &[1, 2, 0, 0, 0]);
    assert_eq!(&after[5..], &[0; 64]);
    assert_eq!(record.into_before(), original);

    // The independently decoded bitmap supplies membership, not the old row.
    reader.source.get_mut()[8123..8192].copy_from_slice(&after);
    reader.source.get_mut()[8192..8196].copy_from_slice(&[5, 1, 0, 0]);
    reader.source.get_mut()[8196] = 0x05;
    assert_eq!(allocation::pages(&mut reader, 1 << 8).unwrap(), [0, 2]);
}

#[test]
fn inline_rebuild_retires_old_relative_bits_and_checks_every_absolute_member() {
    let mut before = file(69, 0);
    before[8128] = 0x40;
    let original = before[8123..8192].to_vec();
    let mut source = Cursor::new(before);
    let mut budget = ResourceBudget::new(1_000_000, 100, 1_000_000);
    let mut reader = Reader {
        source: &mut source,
        header: header(3),
        budget: &mut budget,
        cancelled: &|| false,
        reads: 0,
    };
    let record = QualifiedInlineRecord::capture(&mut reader, 1 << 8).unwrap();
    let mut expected = vec![0; 69];
    expected[1..5].copy_from_slice(&128_u32.to_le_bytes());
    expected[5] = 0x81;
    expected[68] = 0x80;
    assert_eq!(
        record.inline(128, &[128, 135, 639], &mut reader).unwrap(),
        expected
    );
    assert_eq!(record.before(), original);

    // Both physical edge bits are representable. The caller separately chooses
    // the stricter observed shift policy; the constructor does not choose it.
    for (start, members, error) in [
        (129, vec![129], InspectError::Unsupported),
        (128, vec![127], InspectError::Unsupported),
        (128, vec![640], InspectError::Unsupported),
        (128, vec![128, 128], InspectError::Malformed),
        (u32::MAX - 7, vec![], InspectError::Malformed),
    ] {
        assert_eq!(
            record.inline(start, &members, &mut reader).err(),
            Some(error)
        );
        assert_eq!(record.before(), original);
    }
    assert_eq!(record.inline(128, &[], &mut reader).unwrap()[5..], [0; 64]);

    let mut empty = Cursor::new([]);
    let mut exhausted = ResourceBudget::new(0, 100, 1000);
    let mut limited = Reader {
        source: &mut empty,
        header: header(3),
        budget: &mut exhausted,
        cancelled: &|| false,
        reads: 0,
    };
    assert!(matches!(
        record.inline(128, &[128], &mut limited),
        Err(InspectError::Limit(_))
    ));
    limited.cancelled = &|| true;
    assert_eq!(
        record.inline(128, &[128], &mut limited).err(),
        Some(InspectError::Cancelled)
    );
    assert_eq!(record.before(), original);
}

#[test]
fn unknown_width_and_kind_keep_unsupported_while_bad_native_ranges_stay_malformed() {
    for (width, kind) in [(68, 0), (70, 0), (69, 1), (69, 2)] {
        let mut source = Cursor::new(file(width, kind));
        let mut budget = ResourceBudget::new(1_000_000, 100, 1_000_000);
        let mut reader = Reader {
            source: &mut source,
            header: header(3),
            budget: &mut budget,
            cancelled: &|| false,
            reads: 0,
        };
        assert_eq!(
            QualifiedInlineRecord::capture(&mut reader, 1 << 8).err(),
            Some(InspectError::Unsupported)
        );
    }

    let mut bytes = file(69, 0);
    bytes[4096 + 14..4096 + 16].copy_from_slice(&14_u16.to_le_bytes());
    let mut source = Cursor::new(bytes);
    let mut budget = ResourceBudget::new(1_000_000, 100, 1_000_000);
    let mut reader = Reader {
        source: &mut source,
        header: header(3),
        budget: &mut budget,
        cancelled: &|| false,
        reads: 0,
    };
    assert_eq!(
        QualifiedInlineRecord::capture(&mut reader, 1 << 8).err(),
        Some(InspectError::Malformed)
    );
}
