//! Public images and closed metadata purposes use the independent literal model.
use super::{budget, checked, model};
use crate::{
    EditError, InspectError, NativeImage, RowInsert,
    table::mutation::{
        append::State,
        insert::{audit, compose, preflight, prepare},
        variable_split::metadata::Metadata,
    },
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::Resource;
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn requests(values: &[Vec<tablecraft_core::values::Value>]) -> Vec<RowInsert> {
    values.iter().cloned().map(RowInsert::new).collect()
}

#[test]
fn owned_bitmap_promotion_and_later_data_reuse_match_complete_independent_images() {
    for code in [5, 6] {
        for count in [1, 16] {
            let case = model::case(code, count);
            checked(&case.before, &case.old_rows);
            checked(&case.expected, &case.final_rows);
            let requests = requests(&case.values);
            let candidate = capture(&case.before)
                .preview_ace16_insert_rows("Items", &requests, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(candidate.changed_rows(), count);
            assert_eq!(candidate.candidate_bytes().len(), case.expected.len());
            let difference = candidate
                .candidate_bytes()
                .iter()
                .zip(&case.expected)
                .position(|(actual, expected)| actual != expected);
            assert!(
                difference.is_none(),
                "code {code}, count {count}, first difference {difference:?}"
            );
            checked(candidate.candidate_bytes(), &case.final_rows);
        }
    }
}

#[test]
fn owned_promotion_and_reuse_exhaust_each_pool_and_cancel_at_the_terminal_callback() {
    let case = model::case(6, 16);
    checked(&case.before, &case.old_rows);
    checked(&case.expected, &case.final_rows);
    let requests = requests(&case.values);
    let polls = Cell::new(0_usize);
    let mut consumed = budget();
    let healthy = capture(&case.before)
        .preview_ace16_insert_rows("Items", &requests, &mut consumed, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(healthy.candidate_bytes(), case.expected);
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let initial = budget().remaining(resource);
        let used = initial - consumed.remaining(resource);
        assert!(used > 0);
        let mut limited = budget();
        limited.charge(resource, initial - (used - 1)).unwrap();
        let result = capture(&case.before).preview_ace16_insert_rows(
            "Items",
            &requests,
            &mut limited,
            &|| false,
        );
        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == resource)
        );
    }
    let observed = Cell::new(0_usize);
    let result =
        capture(&case.before).preview_ace16_insert_rows("Items", &requests, &mut budget(), &|| {
            observed.set(observed.get() + 1);
            observed.get() == polls.get()
        });
    assert!(matches!(
        result,
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(observed.get(), polls.get());
}

#[test]
fn correlated_closed_purpose_substitution_cannot_turn_owned_metadata_into_data() {
    let case = model::case(6, 16);
    checked(&case.before, &case.old_rows);
    checked(&case.expected, &case.final_rows);
    let requests = requests(&case.values);
    for wrong_purpose in [false, true] {
        let image = capture(&case.before);
        let checked = preflight(&image, "Items", &requests, &mut budget(), &|| false).unwrap();
        let mut stage = prepare(&image, &checked, &requests, &mut budget(), &|| false).unwrap();
        let healthy = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
        assert_eq!(healthy, case.expected);
        assert_eq!(
            audit::prove(
                &image,
                "Items",
                &requests,
                &stage,
                &healthy,
                &mut budget(),
                &|| false
            ),
            Ok(())
        );
        let metadata = stage.data.maps.owned_metadata_page().unwrap();
        assert_eq!((metadata.0, metadata.1, metadata.2), (521, 11 << 8, 0));
        assert_eq!(
            stage
                .data
                .append
                .require_owned_bitmap(521, 10, 11 << 8, 1, &mut budget(), &|| false,),
            Err(EditError::Preservation)
        );

        let mut replacement = State::new(&image, 10);
        replacement
            .reserve_data(520, &mut budget(), &|| false)
            .unwrap();
        if wrong_purpose {
            replacement
                .reserve_data(521, &mut budget(), &|| false)
                .unwrap();
        } else {
            replacement
                .reserve_owned_bitmap(521, 11 << 8, 0, &mut budget(), &|| false)
                .unwrap();
        }
        replacement
            .reserve_data(522, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(replacement.length, stage.data.append.length);
        assert_eq!(
            replacement.reservations_len(),
            stage.data.append.reservations_len()
        );
        replacement.map_edits = std::mem::take(&mut stage.data.append.map_edits);
        let metadata = Metadata::capture_for_insert(
            &image,
            checked.schema(),
            checked.indexes(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        replacement
            .retain_source_metadata(&image, metadata)
            .unwrap();
        stage.data.append = replacement;
        // Exact bytes, full source authority and suffix cardinality are equal.
        // Only the closed purpose at 521 differs in the negative control.
        assert_eq!(
            compose::compose(&image, &stage, &mut budget(), &|| false).err(),
            if wrong_purpose {
                Some(EditError::Preservation)
            } else {
                None
            }
        );
        let result = audit::prove(
            &image,
            "Items",
            &requests,
            &stage,
            &healthy,
            &mut budget(),
            &|| false,
        );
        assert_eq!(
            result,
            if wrong_purpose {
                Err(EditError::Preservation)
            } else {
                Ok(())
            }
        );
        assert_eq!(stage.data.append.length, 523 * PAGE);
    }
}
