//! Healthy full images separate physical inventory and byte authority forgeries.
use super::*;
use crate::table::mutation::insert::{counters, tests::multiple_indexes::model as indexes};

fn requested() -> Vec<RowInsert> {
    [
        indexes::Row {
            id: 4,
            group: Some(10),
            binary: None,
        },
        indexes::Row {
            id: 5,
            group: None,
            binary: Some(vec![]),
        },
    ]
    .into_iter()
    .map(|row| RowInsert::new(row.values()))
    .collect()
}

#[test]
fn physical_plan_order_and_duplicate_alias_work_are_not_candidate_authority() {
    let case = indexes::source(6, false, false);
    let requests = requested();
    let inserted = [
        indexes::Row {
            id: 4,
            group: Some(10),
            binary: None,
        },
        indexes::Row {
            id: 5,
            group: None,
            binary: Some(vec![]),
        },
    ];
    let expected = indexes::expected(&case, &inserted);
    checked(&case.before, 3);
    checked(&expected, 5);
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

    stage.indexes.swap(0, 1);
    let swapped = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    assert_eq!(swapped, healthy);
    assert_eq!(
        prove(
            &image,
            "Items",
            &requests,
            &stage,
            &swapped,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    );
    stage.indexes.swap(0, 1);

    let (_, mut duplicate) = staged(&case.before, &requests);
    stage.indexes.push(duplicate.indexes.remove(1));
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
        Err(EditError::Preservation)
    );
}

#[test]
fn secondary_history_and_unused_leaf_bytes_are_independently_certified() {
    let case = indexes::source(5, false, false);
    let requests = requested();
    let (image, mut stage) = staged(&case.before, &requests);
    let healthy = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    checked(&healthy, 5);
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
    assert_eq!(stage.indexes[1].distinct_added(), 0);
    assert_eq!(stage.counters.len(), 2);

    let offset = 10 * PAGE + 79;
    assert_eq!(&case.before[offset..offset + 4], &29_u32.to_le_bytes());
    stage.counters.push(counters::Patch {
        offset,
        before: 29_u32.to_le_bytes(),
        after: 31_u32.to_le_bytes(),
    });
    let wrong_count = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    checked(&wrong_count, 5);
    assert_eq!(
        prove(
            &image,
            "Items",
            &requests,
            &stage,
            &wrong_count,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    );
    stage.counters.pop();

    let offset = 14 * PAGE + PAGE - 1;
    stage.indexes[1].patches.push(IndexDataPatch {
        offset,
        before: vec![case.before[offset]],
        after: vec![case.before[offset] ^ 1],
    });
    let slack = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    checked(&slack, 5);
    assert_eq!(
        prove(
            &image,
            "Items",
            &requests,
            &stage,
            &slack,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    );
}
