//! Native DELETE requests and complete images from the independently read source.
#[path = "genuine_ordinary_delete/fixtures.rs"]
mod fixtures;
#[path = "genuine_ordinary_delete/recovery.rs"]
mod recovery;
#[path = "genuine_ordinary_delete/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, scope};
use fixtures::{Case, cases, requests};
use journals::Marker;
use std::io::Cursor;
use tablecraft_ace::{
    EditError, NativeImage, RecoverableEditError, RecoverableNativeDeletePlan, RowDelete,
    read_table,
};
use tablecraft_core::limits::ResourceBudget;

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn plan(
    case: &Case,
    work: &mut ResourceBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<RecoverableNativeDeletePlan, RecoverableEditError> {
    image(&case.before).plan_recoverable_ordinary_row_deletes(
        "Sentinel",
        requests(case),
        work,
        cancel,
    )
}

#[test]
#[ignore = "requires all four independently qualified ordinary DELETE images"]
fn two_literal_delete_commits_keep_all_record_property_ole_and_system_bytes() {
    let mut committed = 0;
    for case in cases() {
        let native = image(&case.before);
        let prepared = native
            .prepare_ordinary_row_deletes("Sentinel", &[0], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(prepared[0].expected_bytes(), case.raw);
        let update = plan(&case, &mut budget(), &|| false).unwrap();
        assert_eq!(
            update.intent().encode(&mut budget(), &|| false).unwrap(),
            fixtures::wire(&case)
        );
        assert_eq!(update.before_bytes(), case.before);
        assert_eq!(update.after_bytes(), case.after);
        assert_eq!(update.deleted_rows(), 1);
        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            update
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            1
        );
        assert_eq!(target.bytes.get_ref(), &case.after);
        assert_eq!(
            journal.bytes.get_ref(),
            &journals::journal(&case.before, &case.after, Marker::Committed)
        );
        let rows = read_table(
            &mut Cursor::new(target.bytes.get_ref()),
            scope(),
            "Sentinel",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(rows.rows().is_empty());
        committed += 1;
    }
    assert_eq!(committed, 2);
}

#[test]
#[ignore = "requires all four independently qualified ordinary DELETE images"]
fn empty_duplicate_missing_and_stale_original_requests_never_publish_partial_plans() {
    for case in cases() {
        let native = image(&case.before);
        assert!(
            native
                .prepare_ordinary_row_deletes("Sentinel", &[], &mut budget(), &|| false)
                .unwrap()
                .is_empty()
        );
        assert!(
            native
                .prepare_ordinary_row_deletes("Sentinel", &[0, 0], &mut budget(), &|| false)
                .is_err()
        );
        assert!(matches!(
            native.prepare_ordinary_row_deletes("Sentinel", &[1], &mut budget(), &|| false),
            Err(EditError::Conflict)
        ));
        let empty = native
            .plan_recoverable_ordinary_row_deletes("Sentinel", vec![], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(empty.before_bytes(), case.before);
        assert_eq!(empty.after_bytes(), case.before);
        assert_eq!(empty.deleted_rows(), 0);
        let mut stale = case.raw.clone();
        stale[2] ^= 1;
        assert!(matches!(
            image(&case.before).plan_recoverable_ordinary_row_deletes(
                "Sentinel",
                vec![RowDelete::new(0, stale)],
                &mut budget(),
                &|| false,
            ),
            Err(RecoverableEditError::Edit(EditError::Conflict))
        ));
        assert!(
            image(&case.before)
                .plan_recoverable_ordinary_row_deletes(
                    "Sentinel",
                    vec![
                        RowDelete::new(0, case.raw.clone()),
                        RowDelete::new(0, case.raw.clone())
                    ],
                    &mut budget(),
                    &|| false,
                )
                .is_err()
        );
    }
}
