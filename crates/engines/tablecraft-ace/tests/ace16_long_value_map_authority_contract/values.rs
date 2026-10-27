use super::*;

#[test]
fn empty_cross_catalog_map_rows_are_exclusive_even_without_data_members() {
    for code in [5, 6] {
        let disjoint = fresh(code);
        healthy(&disjoint, 1);
        let result = run(
            &disjoint,
            "Memo",
            &[memo_update(Value::Text("Ā".repeat(33)))],
        )
        .unwrap();
        assert_eq!(
            read(result.candidate_bytes())[0][1],
            Value::Text("Ā".repeat(33))
        );
        assert_eq!(result.candidate_bytes().len(), disjoint.len() + PAGE);
        for (owned, hint) in [
            (OWNED_REFERENCE, OTHER_HINT),
            (OTHER_OWNED, HINT_REFERENCE),
            (OWNED_REFERENCE, HINT_REFERENCE),
            (HINT_REFERENCE, OTHER_HINT),
            (OTHER_OWNED, OWNED_REFERENCE),
        ] {
            let mut aliased = fresh(code);
            bind_other(&mut aliased, owned, hint);
            healthy(&aliased, 1);
            let original = aliased.clone();
            reject_alias(&aliased);
            assert_eq!(aliased, original);
        }
    }
}

#[test]
fn historical_empty_declarations_retain_map_authority_without_live_fields() {
    for code in [5, 6] {
        let mut bytes = fresh(code);
        history(&mut bytes, OWNED_REFERENCE, HISTORY_HINT);
        healthy(&bytes, 1);
        let report = check_table_long_values(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Other",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(report.columns().len(), 2);
        assert_eq!(
            report.columns()[1].status(),
            LongValueCheckStatus::HistoricalDeclaration
        );
        assert_eq!(
            read_table(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Other",
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows()[0][1],
            Value::Text("A".into())
        );
        reject_alias(&bytes);
    }
}

#[test]
fn empty_ordinary_owned_and_candidate_roles_cannot_share_selected_column_rows() {
    for code in [5, 6] {
        let mut owned = fresh(code);
        support::put32(&mut owned, OTHER_DEFINITION * PAGE + 16, 0);
        support::put32(
            &mut owned,
            OTHER_DEFINITION * PAGE + TABLE_USAGE,
            OWNED_REFERENCE,
        );
        other_maps(&mut owned, &[], &[], &[]);
        healthy(&owned, 0);
        reject_alias(&owned);

        let mut hint = fresh(code);
        support::put32(
            &mut hint,
            OTHER_DEFINITION * PAGE + TABLE_HINT,
            HINT_REFERENCE,
        );
        healthy(&hint, 1);
        reject_alias(&hint);
    }
}

#[test]
fn shared_indirect_bitmap_storage_is_refused_without_indirect_selected_lval_maps() {
    for code in [5, 6] {
        let mut disjoint = fresh(code);
        indirect_global_and_other_hint(&mut disjoint, false);
        healthy(&disjoint, 1);
        let candidate = run(
            &disjoint,
            "Memo",
            &[memo_update(Value::Text("Ā".repeat(33)))],
        )
        .unwrap();
        assert_eq!(
            read(candidate.candidate_bytes())[0][1],
            Value::Text("Ā".repeat(33))
        );
        assert_eq!(
            &candidate.candidate_bytes()[OTHER_HINT_BITMAP * PAGE..(OTHER_HINT_BITMAP + 1) * PAGE],
            &disjoint[OTHER_HINT_BITMAP * PAGE..(OTHER_HINT_BITMAP + 1) * PAGE]
        );
        assert_eq!(
            &candidate.candidate_bytes()[OTHER_MAP * PAGE..(OTHER_MAP + 1) * PAGE],
            &disjoint[OTHER_MAP * PAGE..(OTHER_MAP + 1) * PAGE]
        );

        let mut shared = fresh(code);
        indirect_global_and_other_hint(&mut shared, true);
        healthy(&shared, 1);
        // Both metadata rows reference a real bitmap page. The other table's
        // free-space hint is inspected for storage identity, never adopted as
        // the selected table's candidate population.
        reject_alias(&shared);
    }
}

#[test]
fn distinct_map_rows_share_containers_and_real_index_aliases_share_one_owner() {
    for code in [5, 6] {
        let mut bytes = fresh(code);
        indexed_other(&mut bytes);
        healthy(&bytes, 1);
        let report = check_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Other",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(report.indexes().len(), 2);
        assert!(
            report
                .indexes()
                .iter()
                .all(|index| index.status() == tablecraft_ace::IndexCheckStatus::Checked)
        );
        let schema = tablecraft_ace::inspect_table_schema(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Other",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(schema.columns()[0].name(), "Id");
        let candidate = run(&bytes, "Memo", &[memo_update(Value::Text("Ā".repeat(33)))]).unwrap();
        assert_eq!(
            read(candidate.candidate_bytes())[0][1],
            Value::Text("Ā".repeat(33))
        );
        for number in [OTHER_DEFINITION, OTHER_MAP, OTHER_DATA, INDEX_ROOT] {
            assert_eq!(
                &candidate.candidate_bytes()[number * PAGE..(number + 1) * PAGE],
                &bytes[number * PAGE..(number + 1) * PAGE]
            );
        }
        let noop = run(&bytes, "Memo", &[memo_update(Value::Text("A".into()))]).unwrap();
        assert_eq!(noop.candidate_bytes(), bytes);
    }
}

#[test]
fn paid_role_census_cancels_atomically_and_preserves_an_exact_retry() {
    use std::cell::Cell;

    let bytes = fresh(6);
    healthy(&bytes, 1);
    let updates = [memo_update(Value::Text("Ā".repeat(33)))];
    for mut limited in [
        ResourceBudget::new(0, 100_000, 50_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 50_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        let result = capture(&bytes).preview_ace16_long_value_updates(
            "Items",
            "Memo",
            &updates,
            &mut limited,
            &|| false,
        );
        assert!(matches!(
            result,
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let calls = Cell::new(0);
    let accepted = capture(&bytes)
        .preview_ace16_long_value_updates("Items", "Memo", &updates, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let total = calls.get();
    assert!(total > 2);
    calls.set(0);
    let cancelled = capture(&bytes).preview_ace16_long_value_updates(
        "Items",
        "Memo",
        &updates,
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            calls.get() >= total - 1
        },
    );
    assert_eq!(
        cancelled.err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
    assert_eq!(
        run(&bytes, "Memo", &updates).unwrap().candidate_bytes(),
        accepted.candidate_bytes()
    );
}
