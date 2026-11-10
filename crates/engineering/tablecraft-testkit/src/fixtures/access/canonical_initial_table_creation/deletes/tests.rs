//! Independent complete images and literal preservation scope after both renames.
use super::*;
use crate::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::inserting::first_number_row,
    saved_field_rename::fitting::{SourceCase, pair, populated_number_pair},
};
use sha2::{Digest, Sha256};

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn generated_long_row_delete_retains_renames_history_record_and_inactive_key_bytes() {
    for (name, insert_pin, update_pin, delete_pin) in [
        (
            "TaxAmount",
            "d2cdf7f4ffefdc4ef185d7b0a8a071473a6ecf3947ee4aaa808f3658859b08da",
            "544da0e202f81fbca77952c7e8851db8c71c0af40f3935d1dd89ad84d395ae14",
            "359dcac26c0ec392468deafb684725ff35a83cef5c7fbcf629a5c8aedd62ce53",
        ),
        (
            "X",
            "6d052ed5e3e6bddff9df8592feadde689d077d108f9090155d2d59733b9eeb77",
            "afd3c1f9a66ad014fca113bbc08c2c921ed7b4cdeabaa3d53818d9bd0ba2e762",
            "ffc57cc9c3bba7cdcbd732eea578cbf382817df3dc75ef3323fd7d0170d28787",
        ),
    ] {
        let [_, renamed] = pair(SourceCase::NumberFirst, name);
        let inserted = first_number_row(&renamed, &[(0, Some(1)), (1, Some(0))]);
        let updated = first_number_row(&renamed, &[(0, Some(1)), (1, Some(42))]);
        let deleted = only_long_row(&updated);
        assert_eq!(digest(&inserted), insert_pin);
        assert_eq!(digest(&updated), update_pin);
        assert_eq!(digest(&deleted), delete_pin);
        assert_eq!(deleted.len(), updated.len());

        let changes = [
            7 * 4096 + 16..7 * 4096 + 20,
            8 * 4096 + 14..8 * 4096 + 16,
            9 * 4096 + 2..9 * 4096 + 4,
            9 * 4096 + 27..9 * 4096 + 480,
        ];
        for (offset, (before, after)) in updated.iter().zip(&deleted).enumerate() {
            if !changes.iter().any(|span| span.contains(&offset)) {
                assert_eq!(
                    before, after,
                    "outside independent DELETE scope at {offset}"
                );
            }
        }
        assert_eq!(&deleted[7 * 4096 + 20..7 * 4096 + 24], &[1, 0, 0, 0]);
        assert_eq!(&deleted[7 * 4096 + 67..7 * 4096 + 71], &[1, 0, 0, 0]);
        assert_eq!(
            &deleted[9 * 4096 + 480..9 * 4096 + 489],
            &[127, 128, 0, 0, 1, 0, 0, 8, 0]
        );
        assert_eq!(
            &deleted[9 * 4096 - 11..9 * 4096],
            &[2, 0, 1, 0, 0, 0, 42, 0, 0, 0, 3]
        );
    }
}

#[test]
fn dense_nullable_batch_delete_preserves_original_slots_counters_and_inactive_tail() {
    for (name, before_pin, after_pin) in [
        (
            "TaxAmount",
            "c53b03fea441466556958d5312feca6cbf16a9e05fe9617bafb38b8b9064da59",
            "c820cafbecc915a517dc3469abfb3df301b7733151d11854c47a53d525921aa0",
        ),
        (
            "X",
            "44dd0fac9272c7da05e6ab20606a8b09db15a7ecc6fa891108da877e90570ce2",
            "bd0736dc4aec82208570170eb8fd109fa909ab44d88bf99b0900aa0544b71f62",
        ),
    ] {
        let [_, before] = populated_number_pair(name, &[Some(99), None, Some(-7), Some(0)]);
        let after = long_row(&before, 1);
        assert_eq!(digest(&before), before_pin);
        assert_eq!(digest(&after), after_pin);
        assert_eq!(long(&after, 7 * 4096 + 16), 3);
        assert_eq!(long(&after, 7 * 4096 + 20), 4);
        assert_eq!(long(&after, 7 * 4096 + 67), 4);
        assert_eq!(word(&after, 8 * 4096 + 16), 0xc000 | (4096 - 22));

        let allowed = [
            7 * 4096 + 16..7 * 4096 + 20,
            8 * 4096 + 16..8 * 4096 + 18,
            9 * 4096 + 2..9 * 4096 + 4,
            9 * 4096 + 27..9 * 4096 + 507,
        ];
        for (at, (old, new)) in before.iter().zip(&after).enumerate() {
            if !allowed.iter().any(|range| range.contains(&at)) {
                assert_eq!(old, new, "outside original-slot deletion scope at {at}");
            }
        }
        assert_eq!(
            &after[9 * 4096 - 44..9 * 4096],
            &before[9 * 4096 - 44..9 * 4096]
        );
        assert_eq!(
            &after[9 * 4096 + 507..10 * 4096],
            &before[9 * 4096 + 507..10 * 4096]
        );
    }
}
