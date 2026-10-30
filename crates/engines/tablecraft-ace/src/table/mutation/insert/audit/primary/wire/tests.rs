//! Long common prefixes require the actual encoded comparison-width charge.
use super::*;
use crate::NativeImage;
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::multiple_indexes as model;

fn with_reader(run: impl FnOnce(&mut Reader<'_, Cursor<&[u8]>>)) {
    let source = model::source(6, false, false);
    let mut allowance = ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000);
    let image = NativeImage::capture(
        &mut Cursor::new(&source.before),
        CatalogScope::new(1),
        &mut allowance,
        &|| false,
    )
    .unwrap();
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    run(&mut reader);
}

fn always_cancelled() -> bool {
    true
}

fn wide(value: u8, row: u32, reader: &mut Reader<'_, Cursor<&[u8]>>) -> Wire {
    let mut bytes = Vec::new();
    for _ in 0..10 {
        bytes.push(127);
        bytes.extend([0; 8]);
        bytes.push(9);
        bytes.extend([0; 8]);
        bytes.push(8);
    }
    bytes[188] = value;
    bytes.extend(row.to_be_bytes());
    assert_eq!(bytes.len(), 194);
    Wire::read_paid(&bytes, reader).unwrap()
}

#[test]
fn long_guid_tuple_prefixes_refuse_the_old_inline_sort_allowance() {
    with_reader(|reader| {
        let mut entries = (0..16)
            .map(|position| wide(15 - position, u32::from(position) + 1, reader))
            .collect::<Vec<_>>();
        assert!(size_of::<Wire>() < 194);
        let inline_allowance = 16 + 16 * 5 * size_of::<Wire>() as u64;
        assert_eq!(&entries[0][..188], &entries[15][..188]);
        let first = entries[0].to_vec();

        // The old struct-width charge plus the paid entry scan would pass.
        // Sixteen real 194-byte entries require a greater allowance.
        *reader.budget = ResourceBudget::new(1_000_000, 1_000_000, inline_allowance);
        assert!(matches!(
            sort_paid(&mut entries, reader),
            Err(EditError::Inspect(InspectError::Limit(LimitError {
                resource: Resource::Steps,
                ..
            })))
        ));
        assert_eq!(&entries[0][..], first);

        *reader.budget = ResourceBudget::new(1_000_000, 1_000_000, 15536);
        sort_paid(&mut entries, reader).unwrap();
        assert!(entries.windows(2).all(|pair| pair[0] < pair[1]));
    });
}

#[test]
fn short_numeric_wires_keep_the_inline_width_floor_and_poll_cancellation() {
    with_reader(|reader| {
        let mut entries = (0..16_u32)
            .rev()
            .map(|value| {
                let mut bytes = vec![127];
                bytes.extend((value ^ 0x80000000).to_be_bytes());
                bytes.extend((value + 1).to_be_bytes());
                Wire::read(&bytes).unwrap()
            })
            .collect::<Vec<_>>();
        let inline_allowance = 16 + 16 * 5 * size_of::<Wire>() as u64;
        *reader.budget = ResourceBudget::new(1_000_000, 1_000_000, inline_allowance);
        sort_paid(&mut entries, reader).unwrap();
        assert!(entries.windows(2).all(|pair| pair[0] < pair[1]));

        reader.cancelled = &always_cancelled;
        assert!(matches!(
            sort_paid(&mut entries, reader),
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
    });
}

#[test]
fn variable_key_prefix_order_is_decided_before_the_logical_row_trailer() {
    // DESC empty Binary is the complete one-byte key80. EightFF payload bytes
    // form80 + eight00 bytes +f7. Empty is the shorter key, regardless of ROW.
    let empty = Wire::read(&[128, 0, 0, 12, 250]).unwrap();
    let value = Wire::read(&[128, 0, 0, 0, 0, 0, 0, 0, 0, 247, 0, 0, 12, 1]).unwrap();
    assert!(empty[..] > value[..]);
    assert!(empty < value);
    let duplicate = Wire::read(&[128, 0, 0, 12, 251]).unwrap();
    assert!(empty < duplicate);
    assert!(duplicate < value);
    let mut entries = [value, duplicate, empty];
    with_reader(|reader| {
        sort_paid(&mut entries, reader).unwrap();
    });
    assert_eq!(&entries[0][..], &[128, 0, 0, 12, 250]);
    assert_eq!(&entries[1][..], &[128, 0, 0, 12, 251]);
    assert_eq!(entries[2].len(), 14);
}

#[test]
fn branch_wires_exclude_child_bytes_before_key_and_row_comparison() {
    with_reader(|reader| {
        let empty = Wire::read(&[128, 0, 0, 12, 250]).unwrap();
        let value = Wire::read(&[128, 0, 0, 0, 0, 0, 0, 0, 0, 247, 0, 0, 12, 1]).unwrap();
        let first = empty.child_paid(u32::MAX, reader).unwrap();
        let second = value.child_paid(1, reader).unwrap();
        assert!(matches!(first, Wire::BranchInline { .. }));
        assert!(matches!(second, Wire::BranchOwned(_)));
        assert!(first[..] > second[..]);
        assert!(first < second);
        assert_eq!(first.copy_paid(reader).unwrap(), first);
        assert_eq!(second.copy_paid(reader).unwrap(), second);
        assert!(matches!(
            second.copy_paid(reader).unwrap(),
            Wire::BranchOwned(_)
        ));

        let later_row = Wire::read(&[128, 0, 0, 12, 251]).unwrap();
        let later = later_row.child(0).unwrap();
        assert!(first < later);
        let same_bound_other_child = empty.child(0).unwrap();
        assert_ne!(first, same_bound_other_child);
        // Child is only a final physical-byte tie, after identical key/ROW.
        assert!(first > same_bound_other_child);
        assert!(matches!(first.child(1), Err(EditError::Preservation)));
    });
}

#[test]
fn source_leaf_and_branch_decoders_preserve_the_explicit_comparison_shape() {
    with_reader(|reader| {
        let entries = [
            vec![128, 0, 0, 12, 250],
            vec![128, 0, 0, 0, 0, 0, 0, 0, 0, 247, 0, 0, 12, 1],
        ];
        let mut leaf = vec![0; 4096];
        leaf[..4].copy_from_slice(&[4, 1, 13, 14]);
        leaf[27] = 32;
        leaf[29] = 8;
        leaf[480..485].copy_from_slice(&entries[0]);
        leaf[485..499].copy_from_slice(&entries[1]);
        let (decoded, _) = super::super::decode(&leaf, reader).unwrap();
        assert_eq!(&decoded[0][..], entries[0].as_slice());
        assert_eq!(&decoded[1][..], entries[1].as_slice());
        assert!(decoded[0] < decoded[1]);

        let branch_entries = entries
            .iter()
            .enumerate()
            .map(|(ordinal, entry)| {
                let mut bytes = entry.clone();
                bytes.extend((100 - ordinal as u32).to_be_bytes());
                bytes
            })
            .collect::<Vec<_>>();
        let mut branch = vec![0; 4096];
        branch[..4].copy_from_slice(&[3, 1, 5, 14]);
        branch[26] = 1;
        branch[28] = 2;
        branch[30] = 8;
        branch[480..489].copy_from_slice(&branch_entries[0]);
        branch[489..507].copy_from_slice(&branch_entries[1]);
        let (bounds, _) = super::super::decode_page(&branch, true, reader).unwrap();
        assert!(matches!(bounds[0], Wire::BranchInline { .. }));
        assert!(matches!(bounds[1], Wire::BranchOwned(_)));
        assert!(bounds[0] < bounds[1]);
    });
}

#[test]
fn malformed_short_wire_constructors_refuse_before_any_ordering() {
    for length in 0..5 {
        assert!(matches!(
            Wire::read(&[0; 5][..length]),
            Err(EditError::Preservation)
        ));
    }
    with_reader(|reader| {
        for length in 0..9 {
            assert!(matches!(
                Wire::read_branch_paid(&[0; 9][..length], reader),
                Err(EditError::Preservation)
            ));
        }
    });
}
