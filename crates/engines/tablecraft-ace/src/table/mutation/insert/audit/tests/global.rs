//! Actual staged GLOBAL facts must match independent source/frontier replay.
use super::*;
use crate::table::mutation::{
    append::State,
    insert::tests::global_map_growth::{
        self as literal,
        model::{self, Case, Phase},
    },
};

fn prepared(code: u8, phase: Phase) -> (Case, NativeImage, Staged, Vec<RowInsert>) {
    let case = model::case(code, phase, 512, true);
    literal::checked(&case.before, &case.old_rows, case.aliases);
    literal::checked(&case.expected, &case.final_rows, case.aliases);
    let requests = vec![RowInsert::new(case.values.clone())];
    let (image, stage) = staged(&case.before, &requests);
    let after = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    assert_eq!(after, case.expected);
    assert_eq!(stage.data.append.global_bitmap().unwrap().number, 513);
    assert_eq!(
        prove(
            &image,
            "Items",
            &requests,
            &stage,
            &after,
            &mut budget(),
            &|| false
        ),
        Ok(()),
    );
    (case, image, stage, requests)
}

#[test]
fn source_hole_difference_cannot_be_hidden_by_identical_promoted_output() {
    for code in [5, 6] {
        for phase in [
            Phase::Data,
            Phase::Index,
            Phase::LongValue,
            Phase::Container,
        ] {
            let original = model::case(code, phase, 512, false);
            let alternate = model::case(code, phase, 512, true);
            assert_ne!(original.before, alternate.before);
            assert_eq!(original.expected, alternate.expected);
            literal::checked(&original.before, &original.old_rows, original.aliases);
            literal::checked(&alternate.before, &alternate.old_rows, alternate.aliases);
            literal::checked(&original.expected, &original.final_rows, original.aliases);
            let requests = vec![RowInsert::new(original.values.clone())];
            let (alternate_image, alternate_stage) = staged(&alternate.before, &requests);
            let after =
                compose::compose(&alternate_image, &alternate_stage, &mut budget(), &|| false)
                    .unwrap();
            assert_eq!(after, original.expected);
            let image = capture(&original.before);
            assert_eq!(
                prove(
                    &image,
                    "Items",
                    &requests,
                    &alternate_stage,
                    &after,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation),
                "code {code}, phase {phase:?}",
            );
        }
    }
}

#[test]
fn global_row_before_range_pointer_and_kind_are_not_planner_authority() {
    for code in [5, 6] {
        for mutation in 0..4 {
            let (case, image, mut stage, requests) = prepared(code, Phase::Data);
            let record = stage.data.append.global_record_mut().unwrap();
            match mutation {
                0 => record.before[5] ^= 1,
                1 => record.offset += 1,
                2 => record.after[0] = 0,
                3 => record.after[1..5].copy_from_slice(&512_u32.to_le_bytes()),
                _ => unreachable!(),
            }
            let after = if mutation >= 2 {
                let changed = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
                assert_ne!(changed, case.expected);
                changed
            } else {
                case.expected.clone()
            };
            assert_eq!(
                prove(
                    &image,
                    "Items",
                    &requests,
                    &stage,
                    &after,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation),
                "code {code}, record mutation {mutation}",
            );
        }
    }
}

#[test]
fn correlated_high_free_retired_hole_and_bitmap_self_bits_are_exact() {
    for code in [5, 6] {
        for page in [30_usize, 513, 1000, 32703, 32704, 32735] {
            let (case, image, mut stage, requests) = prepared(code, Phase::Data);
            let bitmap = stage.data.append.global_bitmap_mut().unwrap();
            let offset = 4 + page / 8;
            let mask = 1 << (page % 8);
            let original = bitmap.bytes[offset] & mask;
            if !(514..32704).contains(&page) {
                assert_eq!(original, 0);
            } else {
                assert_eq!(original, mask);
            }
            bitmap.bytes[offset] ^= mask;
            let after = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
            assert_ne!(after, case.expected);
            if page != 513 {
                // Neither a free old hole nor a changed future default affects
                // the ordinary native rows. Full row readability is insufficient
                // to certify the requested global transition.
                literal::checked(&after, &case.final_rows, case.aliases);
            }
            assert_eq!(
                prove(
                    &image,
                    "Items",
                    &requests,
                    &stage,
                    &after,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation),
                "code {code}, global page bit {page}",
            );
        }
    }
}

#[test]
fn bitmap_identity_and_closed_global_purpose_cannot_be_substituted() {
    for code in [5, 6] {
        for mutation in 0..3 {
            let (case, image, mut stage, requests) = prepared(code, Phase::Data);
            let bitmap = stage.data.append.global_bitmap_mut().unwrap();
            match mutation {
                0 => bitmap.number = 512,
                1 => bitmap.reference = 11 << 8,
                2 => bitmap.block = 1,
                _ => unreachable!(),
            }
            assert_eq!(
                prove(
                    &image,
                    "Items",
                    &requests,
                    &stage,
                    &case.expected,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation),
            );
        }
        let (case, image, mut stage, requests) = prepared(code, Phase::LongValue);
        let body = stage
            .long_values
            .pages
            .iter()
            .find(|page| page.number == 512)
            .unwrap();
        let mut replacement = State::new(&image, stage.data.append.owner);
        replacement
            .reserve_long_value(
                512,
                body.column,
                body.owned,
                body.candidate,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        replacement
            .reserve_data(513, &mut budget(), &|| false)
            .unwrap();
        replacement.map_edits = std::mem::take(&mut stage.data.append.map_edits);
        assert_eq!(replacement.length, stage.data.append.length);
        assert_eq!(
            replacement.reservations_len(),
            stage.data.append.reservations_len()
        );
        stage.data.append = replacement;
        assert_eq!(
            prove(
                &image,
                "Items",
                &requests,
                &stage,
                &case.expected,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation),
        );
    }
}

#[test]
fn inline_noop_cannot_inherit_another_operations_global_promotion() {
    for code in [5, 6] {
        let (case, image, mut external, _) = prepared(code, Phase::Data);
        let requests = [];
        let (_, mut noop) = staged(&case.before, &requests);
        let unchanged = compose::compose(&image, &noop, &mut budget(), &|| false).unwrap();
        assert_eq!(unchanged, case.before);
        assert!(noop.data.append.global_record().is_none());
        assert!(noop.data.append.global_bitmap().is_none());
        external.data.append.length = noop.data.append.length;
        noop.data.append = external.data.append;
        assert_eq!(
            prove(
                &image,
                "Items",
                &requests,
                &noop,
                &unchanged,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation),
        );
    }
}
