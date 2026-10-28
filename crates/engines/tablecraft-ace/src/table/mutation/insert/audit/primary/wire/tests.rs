//! Long common prefixes require the actual encoded comparison-width charge.
use super::*;
use crate::{NativeImage, table::mutation::insert::tests::multiple_indexes::model};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

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
