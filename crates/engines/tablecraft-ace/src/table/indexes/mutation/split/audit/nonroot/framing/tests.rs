//! Literal capacities distinguish full ROW compression and stable prefix seeds.
use super::*;
use crate::{
    table::indexes::{
        mutation::{NumericKey, profile::SingleProfile, split::borrowed_reference},
        page::KeyRef,
    },
    test_support::split_model,
};
use tablecraft_schema::ScalarType;

fn entries(count: usize, maximum: bool) -> Vec<Entry> {
    (0..count)
        .map(|position| Entry {
            value: KeyRef {
                key: Some(if maximum && position + 1 == count {
                    i64::MAX
                } else {
                    0
                }),
                row: (12_u32 << 8) + u32::try_from(position).unwrap(),
            },
            child: 0,
        })
        .collect()
}

#[test]
fn independent_capacity_uses_full_row_bytes_and_original_or_fresh_seed() {
    let bytes = split_model::source(6, false, false);
    let image = split_model::capture(&bytes);
    let mut source = image.reader();
    let mut budget = split_model::budget();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: &mut budget,
        cancelled: &|| false,
        reads: 0,
    };
    let profile = SingleProfile {
        column: 1,
        kind: NumericKey::new(ScalarType::Int64).unwrap(),
        descending: false,
    };
    // This is a wire-arithmetic test; the reader supplies metering only. No
    // payload is read or mutation-profile eligibility inferred from its table.
    for (count, maximum, seed, prefix, bytes, fits_page) in [
        (900, false, 9, 9, 3609, true),
        (900, false, 0, 11, 1811, true),
        (450, false, 0, 11, 911, true),
        (225, true, 0, 0, 2925, true),
        (225, true, 9, 1, 2701, true),
        (450, true, 0, 1, 5401, false),
        (900, true, 9, 1, 10801, false),
    ] {
        let owned = entries(count, maximum);
        let entries: Vec<_> = owned
            .iter()
            .map(|entry| Entry {
                value: borrowed_reference(&entry.value),
                child: entry.child,
            })
            .collect();
        let measured =
            measure::<_, { encoding::MAX_ENTRY_BYTES }>(&entries, seed, &profile, &mut reader)
                .unwrap();
        assert_eq!((measured.prefix, measured.bytes), (prefix, bytes));
        assert_eq!(
            fits::<_, { encoding::MAX_ENTRY_BYTES }>(&entries, seed, &profile, &mut reader)
                .unwrap(),
            fits_page
        );
    }
}
