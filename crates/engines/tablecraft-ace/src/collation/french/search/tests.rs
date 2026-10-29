use super::*;

#[test]
fn french_pruning_uses_only_the_unchanged_append_only_legacy_primary_stream() {
    // Includes secondary-only modifiers, ignorable units, contextual repeats,
    // trailing spaces, isolated surrogates and a complete supplementary pair.
    let alphabet = [0, 32, 65, 0x00e9, 0x0301, 0x3005, 0x30fc, 0xd800, 0xdc00];
    for first in alphabet {
        for second in alphabet {
            for third in alphabet {
                let input = [first, second, third];
                let mut budget = ResourceBudget::new(1_000_000, 0, 100_000);
                let mut work = key::Work {
                    budget: &mut budget,
                    cancelled: &|| false,
                };
                let complete = units_key(&input, &mut work).unwrap();
                let trimmed = crate::collation::comparison::trim_spaces(&input, &mut work).unwrap();
                let legacy =
                    key::encode_full(GeneralSortFamily::Legacy, trimmed, &mut work).unwrap();
                assert_eq!(complete.primary_length, legacy.primary_length);
                assert_eq!(
                    complete.bytes[..complete.primary_length],
                    legacy.bytes[..legacy.primary_length]
                );

                for end in 0..=input.len() {
                    let prefix = units_key(&input[..end], &mut work).unwrap();
                    assert!(
                        complete.bytes[..complete.primary_length]
                            .starts_with(&prefix.bytes[..prefix.primary_length]),
                        "{input:04x?} at {end}"
                    );
                    let bound: usize = input[..end]
                        .iter()
                        .map(|unit| {
                            key::primary_byte_bound(GeneralSortFamily::Legacy, *unit).unwrap()
                        })
                        .sum();
                    assert!(prefix.primary_length <= bound);
                }
            }
        }
    }
}
