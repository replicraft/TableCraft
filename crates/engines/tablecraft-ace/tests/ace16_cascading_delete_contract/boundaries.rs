use super::{model::*, self_fixture};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, IndexCheckStatus, InspectError, RelationshipCheckStatus, RowDelete, TableRowDeletes,
    WriteBlocker, check_table, check_table_constraints,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const PHYSICAL_INDEX_BYTES: usize = 52;
const PHYSICAL_FLAGS_OFFSET: usize = 46;
const FIRST_KEY_LAST_BYTE: usize = 484;

#[test]
fn original_explicit_seed_conflicts_are_not_hidden_by_closure_deduplication() {
    let model = database(6, true);
    before(&model);
    assert_eq!(
        run(&model, &[(1, &[0, 0])]).err(),
        Some(EditError::Conflict)
    );
    let parents = requests(&model, 1, &[0]);
    let duplicate = [
        TableRowDeletes::new("Parents", &parents),
        TableRowDeletes::new("parents", &[]),
    ];
    assert_eq!(
        capture(&model.bytes)
            .preview_ace16_cascading_deletes(&duplicate, &mut budget(), &|| false)
            .err(),
        Some(EditError::Conflict)
    );
    let stale_child = [stale(&model, 0, 1)];
    let groups = [
        TableRowDeletes::new("Parents", &parents),
        TableRowDeletes::new("Items", &stale_child),
    ];
    assert_eq!(
        capture(&model.bytes)
            .preview_ace16_cascading_deletes(&groups, &mut budget(), &|| false)
            .err(),
        Some(EditError::Conflict)
    );
    let missing = [RowDelete::new(99, model.records[1][0].clone())];
    assert_eq!(
        capture(&model.bytes)
            .preview_ace16_cascading_deletes(
                &[TableRowDeletes::new("Parents", &missing)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Conflict)
    );
    let valid = run(&model, &[(1, &[0])]).unwrap();
    verify(&model, &valid, [&[0, 1], &[0], &[]]);
}

#[test]
fn empty_requests_retain_primary_action_gates_without_inventing_global_activation() {
    let model = database(6, true);
    before(&model);
    for seeds in [vec![], vec![(1, &[][..])], vec![(0, &[][..])]] {
        let candidate = run(&model, &seeds).unwrap();
        assert_eq!(candidate.deleted_rows(), 0);
        assert_eq!(candidate.candidate_bytes(), model.bytes);
    }
    let mut no_primary = database(6, true);
    no_primary.bytes[definition(1) * PAGE_BYTES + no_primary.logical_start + INDEX_KIND] = 0;
    let physical = no_primary.logical_start - PHYSICAL_INDEX_BYTES;
    no_primary.bytes[definition(1) * PAGE_BYTES + physical + PHYSICAL_FLAGS_OFFSET] = 0x81;
    before(&no_primary);
    // Incoming-only child requests do not execute the parent's Cascade lookup.
    // A nullable UNIQUE parent remains sufficient for the existing final RI
    // check; action expansion from that parent requires a genuine primary.
    let child_empty = run(&no_primary, &[(0, &[])]).unwrap();
    assert_eq!(child_empty.deleted_rows(), 0);
    assert_eq!(child_empty.candidate_bytes(), no_primary.bytes);
    let child = run(&no_primary, &[(0, &[0])]).unwrap();
    verify(&no_primary, &child, [&[0], &[], &[]]);
    for selected in [&[][..], &[0][..]] {
        assert_eq!(
            run(&no_primary, &[(1, selected)]).err(),
            Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        );
    }

    let mut set_null = database(6, true);
    rules(&mut set_null, 0x2000, 0x1000, 2, 1);
    before(&set_null);
    let empty = run(&set_null, &[(1, &[])]).unwrap();
    assert_eq!(empty.deleted_rows(), 0);
    assert_eq!(empty.before_bytes(), set_null.bytes);
    assert_eq!(empty.candidate_bytes(), set_null.bytes);

    let mut unenforced = database(6, true);
    rules(&mut unenforced, 2, 2, 0, 0);
    for (target, logical) in [(0, 0), (1, 1), (1, 2), (2, 0)] {
        let at =
            definition(target) * PAGE_BYTES + unenforced.logical_start + logical * LOGICAL_BYTES;
        unenforced.bytes[at + FOREIGN_START..at + INDEX_KIND + 1].fill(0);
    }
    before(&unenforced);
    assert_eq!(
        run(&unenforced, &[(1, &[])]).err(),
        Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    );
}

#[test]
fn valid_self_and_two_table_cycles_refuse_even_when_no_rows_are_selected() {
    let model = cycle();
    before(&model);
    require_primary(&model, 0);
    require_primary(&model, 1);
    for seeds in [vec![(1, &[][..])], vec![(1, &[0][..])]] {
        assert_eq!(
            run(&model, &seeds).err(),
            Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        );
    }

    let bytes = self_fixture::database(6, false, true);
    let checked = check_table(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    check_table_constraints(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        capture(&bytes)
            .preview_ace16_cascading_deletes(
                &[TableRowDeletes::new("Items", &[])],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    );
}

#[test]
fn caller_bounds_and_failure_atomicity_preserve_reusable_original_bytes() {
    let model = database(6, true);
    before(&model);
    let deletes = requests(&model, 1, &[0]);
    let groups = [TableRowDeletes::new("Parents", &deletes)];
    for mut limited in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(matches!(
            capture(&model.bytes).preview_ace16_cascading_deletes(&groups, &mut limited, &|| false),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let observed = Cell::new(0);
    let successful = capture(&model.bytes)
        .preview_ace16_cascading_deletes(&groups, &mut budget(), &|| {
            observed.set(observed.get() + 1);
            false
        })
        .unwrap();
    verify(&model, &successful, [&[0, 1], &[0], &[]]);
    assert!(observed.get() > 3);
    for stop in [
        0,
        observed.get() / 3,
        observed.get() * 2 / 3,
        observed.get() - 1,
    ] {
        let calls = Cell::new(0);
        let cancelled = || {
            let current = calls.get();
            calls.set(current + 1);
            current >= stop
        };
        assert!(matches!(
            capture(&model.bytes).preview_ace16_cascading_deletes(
                &groups,
                &mut budget(),
                &cancelled
            ),
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
    }
    let too_many_tables: Vec<_> = (0..257)
        .map(|_| TableRowDeletes::new("Parents", &[]))
        .collect();
    assert!(matches!(
        capture(&model.bytes).preview_ace16_cascading_deletes(
            &too_many_tables,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Inspect(InspectError::StructureLimit))
    ));
    let too_many_rows: Vec<_> = (0..16_385)
        .map(|_| RowDelete::new(0, model.records[1][0].clone()))
        .collect();
    assert!(matches!(
        capture(&model.bytes).preview_ace16_cascading_deletes(
            &[TableRowDeletes::new("Parents", &too_many_rows)],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Inspect(InspectError::StructureLimit))
    ));
    let retry = run(&model, &[(1, &[0])]).unwrap();
    verify(&model, &retry, [&[0, 1], &[0], &[]]);
}

#[test]
fn checked_source_corruption_and_primary_requiredness_are_not_bypassed() {
    let mut model = database(6, true);
    before(&model);
    // The original leaf's owner remains a valid ordinary table reference, but
    // the selected key no longer matches its stored record. Complete before
    // checks detect this suffix even when only another root was requested.
    model.bytes[root(2) * PAGE_BYTES + FIRST_KEY_LAST_BYTE] ^= 1;
    assert!(
        check_table(
            &mut Cursor::new(&model.bytes),
            CatalogScope::new(1),
            "Other",
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert!(run(&model, &[(1, &[0])]).is_err());

    let mut no_required = database(6, true);
    let physical = no_required.logical_start - PHYSICAL_INDEX_BYTES;
    no_required.bytes[definition(1) * PAGE_BYTES + physical + PHYSICAL_FLAGS_OFFSET] = 0x81;
    before(&no_required);
    // PRIMARY itself supplies requiredness; deleting a physical flag cannot
    // turn a genuine primary into a merely nullable UNIQUE key.
    require_primary(&no_required, 1);
    let candidate = run(&no_required, &[(1, &[0])]).unwrap();
    verify(&no_required, &candidate, [&[0, 1], &[0], &[]]);

    let mut unknown = database(6, true);
    before(&unknown);
    rules(&mut unknown, 0x80000000, 0x1000, 0, 1);
    let checked = check_table(
        &mut Cursor::new(&unknown.bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        checked
            .indexes()
            .iter()
            .any(|index| index.status() == IndexCheckStatus::Unsupported)
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(&unknown.bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        constraints
            .relationships()
            .iter()
            .any(|relationship| relationship.status() == RelationshipCheckStatus::Unsupported)
    );
    assert_eq!(
        run(&unknown, &[(1, &[])]).err(),
        Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    );
}
