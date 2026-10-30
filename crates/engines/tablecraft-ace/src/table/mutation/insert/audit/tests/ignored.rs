//! All-NULL contributions preserve secondary bytes despite ordinary row growth.
use super::*;
use crate::table::mutation::insert::{counters, tests::multiple_indexes::model as indexes};
use tablecraft_testkit::access::multiple_indexes as source_indexes;

fn insertion() -> source_indexes::Row {
    source_indexes::Row {
        id: 4,
        group: None,
        binary: None,
    }
}

fn expected(case: &source_indexes::Case) -> Vec<u8> {
    // Reuse the independent visible-record/primary oracle, while preserving
    // the ignored owner's complete page and historical value by its policy.
    let mut expected = indexes::expected(case, &[insertion()]);
    expected[14 * PAGE..15 * PAGE].copy_from_slice(&case.before[14 * PAGE..15 * PAGE]);
    let counter = 10 * PAGE + 79;
    expected[counter..counter + 4].copy_from_slice(&case.before[counter..counter + 4]);
    expected
}

#[test]
fn ignored_all_null_secondary_recompression_is_semantically_valid_but_refused() {
    for code in [5, 6] {
        let case = indexes::ignored_null_source(code);
        let expected = expected(&case);
        checked(&case.before, 3);
        checked(&expected, 4);
        let requests = vec![RowInsert::new(insertion().values())];
        let (image, mut stage) = staged(&case.before, &requests);
        let healthy = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
        assert_eq!(healthy, expected);
        prove(
            &image,
            "Items",
            &requests,
            &stage,
            &healthy,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(stage.indexes[1].patches.is_empty());
        assert!(stage.indexes[1].added.is_empty());
        assert_eq!(stage.indexes[1].distinct_added(), 0);

        let mut wires = Vec::new();
        for (row, reference) in &case.rows {
            if let Some(value) = row.group {
                let mut wire = vec![127];
                wire.extend(((value as u32) ^ 0x80000000).to_be_bytes());
                wire.extend(reference.to_be_bytes());
                wires.push(wire);
            }
        }
        wires.sort();
        assert_eq!(wires.len(), 2);
        let start = 14 * PAGE;
        let mut root = case.before[start..start + PAGE].to_vec();
        assert_eq!(&root[24..26], &[0, 0]);
        root[27..480].fill(0);
        let mut active = 0;
        for (position, wire) in wires.iter().enumerate() {
            let suffix = &wire[usize::from(position != 0)..];
            root[480 + active..480 + active + suffix.len()].copy_from_slice(suffix);
            active += suffix.len();
            root[27 + active / 8] |= 1 << (active % 8);
        }
        root[2..4].copy_from_slice(&((3616 - active) as u16).to_le_bytes());
        root[24..26].copy_from_slice(&1_u16.to_le_bytes());
        for range in [2..4, 24..26, 27..480 + active] {
            stage.indexes[1].patches.push(IndexDataPatch {
                offset: start + range.start,
                before: case.before[start + range.start..start + range.end].to_vec(),
                after: root[range].to_vec(),
            });
        }
        let forged = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
        checked(&forged, 4);
        assert_ne!(forged, healthy);
        assert_eq!(
            prove(
                &image,
                "Items",
                &requests,
                &stage,
                &forged,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        );
    }
}

#[test]
fn omitted_null_does_not_create_a_historical_key_family() {
    let case = indexes::ignored_null_source(6);
    let expected = expected(&case);
    checked(&expected, 4);
    let requests = vec![RowInsert::new(insertion().values())];
    let (image, mut stage) = staged(&case.before, &requests);
    let healthy = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    assert_eq!(healthy, expected);
    prove(
        &image,
        "Items",
        &requests,
        &stage,
        &healthy,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(stage.counters.len(), 2);
    let offset = 10 * PAGE + 79;
    assert_eq!(&case.before[offset..offset + 4], &29_u32.to_le_bytes());
    stage.counters.push(counters::Patch {
        offset,
        before: 29_u32.to_le_bytes(),
        after: 30_u32.to_le_bytes(),
    });
    let forged = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    checked(&forged, 4);
    assert_eq!(
        prove(
            &image,
            "Items",
            &requests,
            &stage,
            &forged,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    );
}
