//! Literal growth images distinguish byte, column-map and reservation authority.
use super::*;
use crate::table::mutation::{
    append::State,
    insert::{
        long_values::BitmapPage,
        tests::{long_value_inserts::growth::model as growth, multiple_indexes::model as ordinary},
    },
};
use tablecraft_testkit::access::multiple_indexes as source_ordinary;

fn prepared_growth(
    code: u8,
    length: usize,
) -> (growth::Case, NativeImage, Staged, Vec<RowInsert>, Vec<u8>) {
    let case = growth::source_and_expected(code, 528, length, false);
    growth::checked(&case.before, &growth::rows(None));
    growth::checked(&case.expected, &growth::rows(Some(&case.value)));
    assert_eq!(case.bitmaps, [529]);
    let requests = vec![RowInsert::new(vec![
        Value::Int32(10),
        Value::Int32(100),
        Value::Binary(case.value.clone()),
    ])];
    let (image, stage) = staged(&case.before, &requests);
    let after = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    assert_eq!(after, case.expected);
    assert_eq!(stage.long_values.metadata.len(), 1);
    assert_eq!(stage.long_values.metadata[0].number, 529);
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
    (case, image, stage, requests, after)
}

fn record_start(bytes: &[u8], reference: u32) -> usize {
    let page = (reference >> 8) as usize;
    let slot = (reference & 255) as usize;
    let at = page * PAGE + 14 + slot * 2;
    let word = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    assert_eq!(word & 0xe000, 0);
    page * PAGE + usize::from(word & 0x1fff)
}

#[test]
fn owned_bitmap_membership_excludes_itself_and_foreign_page_roles() {
    for code in [5, 6] {
        for member in [529_u32, 12, 13] {
            let (_, image, mut stage, requests, healthy) = prepared_growth(code, 65);
            let bitmap = &mut stage.long_values.metadata[0];
            assert_eq!(bitmap.number, 529);
            let offset = 4 + member as usize / 8;
            let mask = 1 << (member % 8);
            assert_eq!(bitmap.bytes[offset] & mask, 0);
            bitmap.bytes[offset] |= mask;
            let changed = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
            assert_ne!(changed, healthy);
            assert_eq!(
                changed[529 * PAGE + offset],
                healthy[529 * PAGE + offset] | mask
            );
            assert_eq!(
                prove(
                    &image,
                    "Items",
                    &requests,
                    &stage,
                    &changed,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation),
                "code{code}, foreign member{member}",
            );
        }
    }
}

#[test]
fn bitmap_column_map_block_and_page_identity_are_not_authority() {
    for code in [5, 6] {
        let (_, image, mut stage, requests, healthy) = prepared_growth(code, 65);
        let bitmap = &stage.long_values.metadata[0];
        let original = (bitmap.number, bitmap.column, bitmap.owned, bitmap.block);
        assert_eq!(original, (529, 2, (11 << 8) | 2, 0));
        for mutation in 0..4 {
            let bitmap = &mut stage.long_values.metadata[0];
            match mutation {
                0 => bitmap.column = 1,
                1 => bitmap.owned = 11 << 8,
                2 => bitmap.block = 1,
                3 => bitmap.number = 528,
                _ => unreachable!(),
            }
            // The candidate remains the exact healthy image. Only staged
            // identity changes, so neither parsing nor byte drift is the cause.
            assert_eq!(
                prove(
                    &image,
                    "Items",
                    &requests,
                    &stage,
                    &healthy,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation),
                "code{code}, identity mutation{mutation}",
            );
            let bitmap = &mut stage.long_values.metadata[0];
            (bitmap.number, bitmap.column, bitmap.owned, bitmap.block) = original;
        }
    }
}

#[test]
fn promoted_owned_record_keeps_original_before_and_exact_bitmap_pointer() {
    for code in [5, 6] {
        let (case, image, mut stage, requests, healthy) = prepared_growth(code, 65);
        let reference = stage.long_values.metadata[0].owned;
        let start = record_start(&case.before, reference);
        assert_eq!(case.before[start], 0);
        assert_eq!(healthy[start], 1);
        assert_eq!(&healthy[start + 1..start + 5], &529_u32.to_le_bytes());
        let edit = stage
            .data
            .append
            .map_edits
            .iter_mut()
            .find(|edit| edit.offset == start)
            .unwrap();
        let original_before = edit.before;
        edit.before ^= 1;
        assert_eq!(
            prove(
                &image,
                "Items",
                &requests,
                &stage,
                &healthy,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation),
            "source before byte code{code}",
        );
        stage
            .data
            .append
            .map_edits
            .iter_mut()
            .find(|edit| edit.offset == start)
            .unwrap()
            .before = original_before;
        let pointer = stage
            .data
            .append
            .map_edits
            .iter_mut()
            .find(|edit| edit.offset == start + 1)
            .unwrap();
        assert_eq!(pointer.after, 0x11);
        pointer.after = 0x12; // Pointer530 is not the independently allocated bitmap529.
        let changed = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
        assert_ne!(changed, healthy);
        assert_eq!(&changed[start + 1..start + 5], &530_u32.to_le_bytes());
        assert_eq!(
            prove(
                &image,
                "Items",
                &requests,
                &stage,
                &changed,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation),
            "owned pointer code{code}",
        );
    }
}

#[test]
fn hidden_metadata_and_extra_long_value_reservations_do_not_extend_the_suffix() {
    for code in [5, 6] {
        let (_, image, mut stage, requests, healthy) = prepared_growth(code, 65);
        let bitmap = &stage.long_values.metadata[0];
        stage.long_values.metadata.push(BitmapPage {
            number: 527,
            column: bitmap.column,
            owned: bitmap.owned,
            block: bitmap.block,
            bytes: bitmap.bytes.clone(),
        });
        let unchanged = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
        assert_eq!(unchanged, healthy);
        assert_eq!(
            prove(
                &image,
                "Items",
                &requests,
                &stage,
                &unchanged,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation),
        );
        stage.long_values.metadata.pop().unwrap();
        let page = &stage.long_values.pages[0];
        let identity = (page.column, page.owned, page.candidate);
        let length = stage.data.append.length;
        let next = stage.data.append.next().unwrap();
        assert_eq!(next, 530);
        stage
            .data
            .append
            .reserve_long_value(
                next,
                identity.0,
                identity.1,
                identity.2,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        stage.data.append.length = length;
        let unchanged = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
        assert_eq!(unchanged, healthy);
        assert_eq!(
            prove(
                &image,
                "Items",
                &requests,
                &stage,
                &unchanged,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation),
        );
    }
}

#[test]
fn owned_bitmap_reservation_is_distinct_from_an_ordinary_data_purpose() {
    for code in [5, 6] {
        let (_, image, mut stage, requests, healthy) = prepared_growth(code, 65);
        // The plan also retains the visited original page16. Select the
        // appended body by its literal identity rather than plan position.
        let page = stage
            .long_values
            .pages
            .iter()
            .find(|page| page.number == 528)
            .unwrap();
        let mut replacement = State::new(&image, stage.data.append.owner);
        replacement
            .reserve_long_value(
                528,
                page.column,
                page.owned,
                page.candidate,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        replacement
            .reserve_data(529, &mut budget(), &|| false)
            .unwrap();
        replacement.map_edits = std::mem::take(&mut stage.data.append.map_edits);
        assert_eq!(replacement.length, stage.data.append.length);
        assert_eq!(
            replacement.reservations_len(),
            stage.data.append.reservations_len()
        );
        assert_eq!(replacement.long_values().count(), 1);
        assert_eq!(replacement.long_value_owned_bitmaps().count(), 0);
        stage.data.append = replacement;
        // Exact bytes and reservation cardinality are unchanged; the bitmap's
        // closed role cannot be substituted with an ordinary DATA reservation.
        assert_eq!(
            prove(
                &image,
                "Items",
                &requests,
                &stage,
                &healthy,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation),
        );
    }
}

#[test]
fn chained_growth_cannot_follow_an_owned_bitmap_as_the_next_lval_row() {
    for code in [5, 6] {
        let (case, image, mut stage, requests, healthy) = prepared_growth(code, 8145);
        assert_eq!(case.bodies, [528, 530, 531]);
        let page = stage
            .long_values
            .pages
            .iter_mut()
            .find(|page| page.number == 528)
            .unwrap();
        let start = usize::from(u16::from_le_bytes(page.after[14..16].try_into().unwrap()));
        assert_eq!(&page.after[start..start + 4], &(530_u32 << 8).to_le_bytes());
        page.after[start..start + 4].copy_from_slice(&(529_u32 << 8).to_le_bytes());
        let changed = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
        assert_ne!(changed, healthy);
        assert_eq!(
            prove(
                &image,
                "Items",
                &requests,
                &stage,
                &changed,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation),
        );
    }
}

#[test]
fn no_long_declaration_or_no_external_request_cannot_hide_owned_bitmap_metadata() {
    let case = source_ordinary::source(6, false, false);
    let added = source_ordinary::Row {
        id: 4,
        group: Some(30),
        binary: Some(vec![0x63]),
    };
    let expected = ordinary::expected(&case, std::slice::from_ref(&added));
    checked(&case.before, 3);
    checked(&expected, 4);
    let requests = vec![RowInsert::new(added.values())];
    let (image, mut stage) = staged(&case.before, &requests);
    let healthy = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    assert_eq!(healthy, expected);
    assert_eq!(
        prove(
            &image,
            "Items",
            &requests,
            &stage,
            &healthy,
            &mut budget(),
            &|| false
        ),
        Ok(()),
    );
    let mut bitmap = vec![0; PAGE];
    bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
    stage.long_values.metadata.push(BitmapPage {
        number: 9,
        column: 2,
        owned: 11 << 8,
        block: 0,
        bytes: bitmap.clone(),
    });
    let unchanged = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    assert_eq!(unchanged, healthy);
    assert_eq!(
        prove(
            &image,
            "Items",
            &requests,
            &stage,
            &unchanged,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation),
    );

    let case = growth::source_and_expected(6, 528, 65, false);
    growth::checked(&case.before, &growth::rows(None));
    let requests = [];
    let (image, mut stage) = staged(&case.before, &requests);
    let healthy = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    assert_eq!(healthy, case.before);
    assert_eq!(
        prove(
            &image,
            "Items",
            &requests,
            &stage,
            &healthy,
            &mut budget(),
            &|| false
        ),
        Ok(()),
    );
    stage.long_values.metadata.push(BitmapPage {
        number: 529,
        column: 2,
        owned: (11 << 8) | 2,
        block: 0,
        bytes: bitmap,
    });
    let unchanged = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    assert_eq!(unchanged, healthy);
    assert_eq!(
        prove(
            &image,
            "Items",
            &requests,
            &stage,
            &unchanged,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation),
    );
}
