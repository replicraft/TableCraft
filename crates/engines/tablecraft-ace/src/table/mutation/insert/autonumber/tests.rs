//! Reader-valid correlated candidates must still prove original generation intent.
use super::{AutoNumberRowInsert, IncrementingAutoNumber, audit::Operation, counter};
use crate::{
    EditError, IndexCheckStatus, NativeImage, RuleCheckStatus, TableCheck, check_table_constraints,
    read_table,
    table::mutation::{
        gates,
        insert::{
            RowInsert, Staged, audit, compose, prepare_with, record,
            tests::{incrementing_autonumber::model as literal, model as original},
        },
    },
};
use literal::{Case, Row};
use std::io::Cursor;
use tablecraft_model::CatalogScope;

struct Prepared {
    image: NativeImage,
    checked: TableCheck,
    staged: Staged,
}

fn additions(first: i32) -> Vec<Row> {
    vec![
        Row {
            earlier: -11,
            id: first,
            score: Some(20),
        },
        Row {
            earlier: -12,
            id: first + 1,
            score: None,
        },
    ]
}

fn requests(rows: &[Row]) -> Vec<AutoNumberRowInsert> {
    rows.iter()
        .map(|row| AutoNumberRowInsert::new(row.companions()))
        .collect()
}

fn stage(case: &Case, rows: &[Row]) -> Prepared {
    let image = original::capture(&case.before);
    let checked =
        gates::before_incrementing_insert(&image, "Items", &mut original::budget(), &|| false)
            .unwrap();
    let source = counter::bind(
        &image,
        &checked,
        IncrementingAutoNumber::new(case.last),
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let profile = record::Profile::bind_incrementing(
        checked.schema(),
        source.column,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let expanded: Vec<_> = rows
        .iter()
        .map(|row| RowInsert::new(row.values()))
        .collect();
    let mut staged = prepare_with(
        &image,
        &checked,
        &expanded,
        source.column,
        profile,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    staged.autonumber =
        Some(counter::prepare(source, rows.len(), &mut original::budget(), &|| false).unwrap());
    Prepared {
        image,
        checked,
        staged,
    }
}

fn checked(bytes: &[u8], rows: &[Row]) {
    let report = original::checked(bytes, rows.len(), 4);
    assert!(
        report
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let snapshot = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        snapshot.rows(),
        rows.iter().map(Row::values).collect::<Vec<_>>()
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_ne!(constraints.table_validation(), RuleCheckStatus::Unsupported);
    for field in constraints.fields() {
        assert!(
            ![field.required(), field.empty_text(), field.validation()]
                .contains(&RuleCheckStatus::Unsupported)
        );
    }
}

fn healthy(case: &Case, rows: &[Row], requests: &[AutoNumberRowInsert]) -> Prepared {
    // The independent complete image exists before construction is invoked.
    let expected = literal::expected(case, rows);
    let mut final_rows = literal::live(case);
    final_rows.extend(rows.iter().cloned());
    checked(&case.before, &literal::live(case));
    checked(&expected, &final_rows);
    let prepared = stage(case, rows);
    let after = compose::compose(
        &prepared.image,
        &prepared.staged,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(after, expected);
    audit::prove_incrementing(
        &prepared.image,
        "Items",
        (requests, IncrementingAutoNumber::new(case.last)),
        &prepared.staged,
        &after,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    prepared
}

#[test]
fn original_policy_rejects_correlated_wrong_ids_counter_and_valid_index_ledgers() {
    let case = literal::source(6, literal::State::HighWater);
    let rows = additions(261);
    let original_requests = requests(&rows);
    let _healthy = healthy(&case, &rows, &original_requests);

    // These different IDs are unique and agree across DATA, both physical index
    // plans and the staged counter. Only original request/policy authority fails.
    let wrong = additions(262);
    let mut forged = stage(&case, &wrong);
    let plan = forged.staged.autonumber.as_mut().unwrap();
    plan.values = vec![262, 263];
    plan.after = 263_i32.to_le_bytes();
    let after = compose::compose(
        &forged.image,
        &forged.staged,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let mut final_rows = literal::live(&case);
    final_rows.extend(wrong);
    checked(&after, &final_rows);
    assert_eq!(
        audit::prove_incrementing(
            &forged.image,
            "Items",
            (&original_requests, IncrementingAutoNumber::new(case.last)),
            &forged.staged,
            &after,
            &mut original::budget(),
            &|| false,
        )
        .err(),
        Some(EditError::Preservation),
    );
}

#[test]
fn missing_counter_and_wrong_returned_key_order_fail_independent_publication_proof() {
    let case = literal::source(5, literal::State::HighWater);
    let rows = additions(261);
    let original_requests = requests(&rows);
    let prepared = healthy(&case, &rows, &original_requests);
    let plan = prepared.staged.autonumber.as_ref().unwrap();
    let mut after = compose::compose(
        &prepared.image,
        &prepared.staged,
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    after[plan.source.offset..plan.source.offset + 4].copy_from_slice(&plan.source.before);
    let mut final_rows = literal::live(&case);
    final_rows.extend(rows);
    checked(&after, &final_rows);
    assert_eq!(
        audit::prove_incrementing(
            &prepared.image,
            "Items",
            (&original_requests, IncrementingAutoNumber::new(case.last)),
            &prepared.staged,
            &after,
            &mut original::budget(),
            &|| false,
        )
        .err(),
        Some(EditError::Preservation),
    );

    for returned in [&[262, 261][..], &[261][..], &[261, 263][..]] {
        let operation = Operation {
            image: &prepared.image,
            checked: &prepared.checked,
            requests: &original_requests,
            policy: IncrementingAutoNumber::new(case.last),
            plan,
            returned,
        };
        assert_eq!(
            operation.rows(&mut original::budget(), &|| false).err(),
            Some(EditError::Preservation),
        );
        let mut proof = case.before.clone();
        assert_eq!(
            operation
                .apply(&mut proof, &mut original::budget(), &|| false)
                .err(),
            Some(EditError::Preservation),
        );
        assert_eq!(proof, case.before);
    }
}

#[test]
fn empty_maximum_request_has_no_generated_values_or_counter_patch() {
    let case = literal::source(6, literal::State::Maximum);
    let prepared = healthy(&case, &[], &[]);
    let plan = prepared.staged.autonumber.as_ref().unwrap();
    assert!(plan.values.is_empty());
    assert!(plan.patch().is_none());
    let operation = Operation {
        image: &prepared.image,
        checked: &prepared.checked,
        requests: &[],
        policy: IncrementingAutoNumber::new(i32::MAX),
        plan,
        returned: &[],
    };
    assert!(
        operation
            .rows(&mut original::budget(), &|| false)
            .unwrap()
            .is_empty()
    );
    let mut proof = case.before.clone();
    operation
        .apply(&mut proof, &mut original::budget(), &|| false)
        .unwrap();
    assert_eq!(proof, case.before);
}
