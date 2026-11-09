use super::{SourceCase, Variant, remove_nonprimary, source};
#[cfg(any(feature = "access-support", feature = "cli-support"))]
use sha2::{Digest, Sha256};

#[cfg(any(feature = "access-support", feature = "cli-support"))]
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(any(feature = "access-support", feature = "cli-support"))]
#[test]
fn complete_raw_hypotheses_match_four_frozen_inert_reader_images() {
    for (case, field, before_pin, td_pin, live_pin) in [
        (
            SourceCase::NumberHistory,
            "Score",
            "6f9b095ddf3d724e34d5af98bea4e8ba88bc2b2bc70ade10814f85c27770c620",
            "ed4149ecab4ad9cc995728f13f45de8a7f484f959ccea5f253703bf0c750cf09",
            "6c08831dfe4d305064837d93a8d1942a1a87b691ca80e4834a130c202c72923b",
        ),
        (
            SourceCase::MixedTextHistory,
            "Label",
            "c28d3c1bf1de4e75f894ac80ae332aa2a5b4b8c3c9111700d989222b8d6267c3",
            "cca0263c25b7025771866666ff8631099a85b82c08003eb5c140ba6f21b44932",
            "d9432f5edadc0bfcd60551f747be5e6ac630585f5cfa78fffdced6f7e875b935",
        ),
    ] {
        let before = source(case);
        assert_eq!(hash(&before), before_pin);
        assert_eq!(
            hash(&remove_nonprimary(&before, field, Variant::DeclarationOnly)),
            td_pin
        );
        assert_eq!(
            hash(&remove_nonprimary(
                &before,
                field,
                Variant::AppendedGroupRemoved
            )),
            live_pin
        );
    }
}

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

#[test]
fn both_hypotheses_preserve_native_highwater_old_property_rows_and_unrelated_pages() {
    for (case, field, old_count, variables, numbers) in [
        (SourceCase::NumberHistory, "Score", 3, 0, vec![0, 2]),
        (SourceCase::MixedTextHistory, "Label", 4, 2, vec![0, 2, 3]),
    ] {
        let before = source(case);
        let property = &before[10 * 4096..11 * 4096];
        let old_rows = word(property, 12);
        let old_low = word(property, 14 + (old_rows - 1) * 2);
        for variant in [Variant::DeclarationOnly, Variant::AppendedGroupRemoved] {
            let after = remove_nonprimary(&before, field, variant);
            let definition = &after[7 * 4096..8 * 4096];
            assert_eq!(word(definition, 41), old_count);
            assert_eq!(word(definition, 43), variables);
            assert_eq!(word(definition, 45), old_count - 1);
            for (index, number) in numbers.iter().enumerate() {
                assert_eq!(word(definition, 75 + index * 25 + 5), *number);
                assert_eq!(word(definition, 75 + index * 25 + 9), *number);
            }
            assert_eq!(&after[8 * 4096..10 * 4096], &before[8 * 4096..10 * 4096]);
            assert_eq!(&after[3 * 4096..4 * 4096], &before[3 * 4096..4 * 4096]);
            let new_property = &after[10 * 4096..11 * 4096];
            assert_eq!(&new_property[old_low..], &property[old_low..]);
            assert_eq!(
                &new_property[14..14 + old_rows * 2],
                &property[14..14 + old_rows * 2]
            );
            for page in [0, 1, 2, 3, 5, 6, 8, 9] {
                assert_eq!(
                    &after[page * 4096..(page + 1) * 4096],
                    &before[page * 4096..(page + 1) * 4096]
                );
            }
        }
    }
}

#[test]
#[should_panic]
fn raw_recipe_refuses_primary_field_removal() {
    let before = source(SourceCase::NumberHistory);
    remove_nonprimary(&before, "ID", Variant::DeclarationOnly);
}
