#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "ace16_restrict_delete_contract/model.rs"]
mod model;
mod support;

use model::{budget, capture, requests, table};
use tablecraft_ace::{
    EditError, InspectError, NativeImage, RowDelete, TableRowDeletes, read_table,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

#[test]
fn partial_null_composite_associations_use_original_rows_and_final_batch_membership() {
    for code in [5, 6] {
        let model = model::database(code, true, true, false);
        let children = requests(&model, 0, &[0, 1, 3]);
        let parents = requests(&model, 1, &[0, 1]);
        let other = requests(&model, 2, &[0]);
        let groups = [
            TableRowDeletes::new(table(1), &parents),
            TableRowDeletes::new(table(0), &children),
            TableRowDeletes::new(table(2), &other),
        ];
        let result = capture(&model.bytes)
            .preview_ace16_batch_deletes(&groups, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(result.before_bytes(), model.bytes);
        assert_eq!(result.deleted_rows(), 6);
        for (index, expected) in [
            (0, vec![model.rows[0][2].clone()]),
            (1, vec![model.rows[1][2].clone()]),
            (2, vec![]),
        ] {
            let actual = read_table(
                &mut std::io::Cursor::new(result.candidate_bytes()),
                CatalogScope::new(1),
                table(index),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(actual.rows(), expected);
        }
        let reverse = [
            TableRowDeletes::new(table(2), &other),
            TableRowDeletes::new(table(0), &children),
            TableRowDeletes::new(table(1), &parents),
        ];
        let reordered = capture(&model.bytes)
            .preview_ace16_batch_deletes(&reverse, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(result.candidate_bytes(), reordered.candidate_bytes());
    }
}

#[test]
fn cumulative_batch_limits_and_all_resource_pools_refuse_without_a_candidate() {
    let model = model::database(6, false, false, false);
    let children = requests(&model, 0, &[0]);
    let groups = [
        TableRowDeletes::new(table(0), &children),
        TableRowDeletes::new(table(1), &[]),
    ];
    for mut allowance in [
        ResourceBudget::new(0, 100_000, 40_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 40_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        let result =
            capture(&model.bytes).preview_ace16_batch_deletes(&groups, &mut allowance, &|| false);
        assert!(matches!(
            result,
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let empty_groups = (0..=NativeImage::MAX_DELETE_BATCH_TABLES)
        .map(|_| TableRowDeletes::new(table(0), &[]))
        .collect::<Vec<_>>();
    let result =
        capture(&model.bytes).preview_ace16_batch_deletes(&empty_groups, &mut budget(), &|| false);
    assert_eq!(
        result.err(),
        Some(EditError::Inspect(InspectError::StructureLimit))
    );

    let excess = (0..NativeImage::MAX_DELETE_BATCH_ROWS)
        .map(|_| RowDelete::new(0, vec![]))
        .collect::<Vec<_>>();
    let oversized = [
        TableRowDeletes::new(table(0), &children),
        TableRowDeletes::new(table(1), &excess),
    ];
    let result =
        capture(&model.bytes).preview_ace16_batch_deletes(&oversized, &mut budget(), &|| false);
    assert_eq!(
        result.err(),
        Some(EditError::Inspect(InspectError::StructureLimit))
    );
    assert!(
        capture(&model.bytes)
            .preview_ace16_batch_deletes(&groups, &mut budget(), &|| false)
            .is_ok()
    );
    let empty = capture(&model.bytes)
        .preview_ace16_batch_deletes(&[], &mut budget(), &|| false)
        .unwrap();
    assert_eq!(empty.deleted_rows(), 0);
    assert_eq!(empty.candidate_bytes(), model.bytes);
}
