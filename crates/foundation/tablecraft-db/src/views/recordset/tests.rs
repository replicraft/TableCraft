use super::*;
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaError, TypeDescriptor};

fn budget() -> ResourceBudget {
    ResourceBudget::new(0, 0, 100_000)
}

fn schema() -> Arc<RecordsetSchema> {
    Arc::new(
        RecordsetSchema::new(
            vec![
                Column::with_type("FileName".into(), TypeDescriptor::text(2).unwrap(), true)
                    .unwrap(),
                Column::new("FileData".into(), ScalarType::Binary, true).unwrap(),
            ],
            vec![
                MemberAvailability::Available,
                MemberAvailability::Uninspected,
            ],
            NameComparison::Exact,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    )
}

#[test]
fn complete_children_preserve_null_empty_unicode_and_uninspected_payloads() {
    let rows = vec![
        vec![StoredCell::Value(Value::Null), StoredCell::Uninspected],
        vec![
            StoredCell::Value(Value::Text(String::new())),
            StoredCell::Uninspected,
        ],
        vec![
            StoredCell::Value(Value::Text("雪🦀".into())),
            StoredCell::Uninspected,
        ],
        vec![StoredCell::Value(Value::Null), StoredCell::Uninspected],
    ];
    let records = OwnedRecordset::new(
        schema(),
        rows,
        RecordsetPresence::Assigned,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(records.len(), 4);
    assert!(!records.is_empty());
    assert_eq!(records.presence(), RecordsetPresence::Assigned);
    assert_eq!(records.schema().columns()[0].name(), "FileName");
    assert_eq!(
        records.schema().availability()[1],
        MemberAvailability::Uninspected
    );
    assert_eq!(records.rows()[0][0], StoredCell::Value(Value::Null));
    assert_eq!(
        records.rows()[1][0],
        StoredCell::Value(Value::Text(String::new()))
    );
    assert_eq!(
        records.rows()[2][0],
        StoredCell::Value(Value::Text("雪🦀".into()))
    );
    assert_eq!(records.rows()[3][1], StoredCell::Uninspected);
}

#[test]
fn empty_assignment_is_independent_of_schema_and_unassigned_cannot_contain_rows() {
    for presence in [RecordsetPresence::Assigned, RecordsetPresence::Unassigned] {
        let records =
            OwnedRecordset::new(schema(), Vec::new(), presence, &mut budget(), &|| false).unwrap();
        assert!(records.is_empty());
        assert_eq!(records.presence(), presence);
        assert_eq!(records.schema().columns().len(), 2);
    }
    let rows = vec![vec![
        StoredCell::Value(Value::Null),
        StoredCell::Uninspected,
    ]];
    assert_eq!(
        OwnedRecordset::new(
            schema(),
            rows,
            RecordsetPresence::Unassigned,
            &mut budget(),
            &|| false
        )
        .err(),
        Some(RecordsetError::Presence),
    );
}

#[test]
fn availability_never_substitutes_null_and_inspected_values_stay_exact() {
    for rows in [
        vec![vec![StoredCell::Uninspected, StoredCell::Uninspected]],
        vec![vec![
            StoredCell::Value(Value::Null),
            StoredCell::Value(Value::Null),
        ]],
    ] {
        assert_eq!(
            OwnedRecordset::new(
                schema(),
                rows,
                RecordsetPresence::Assigned,
                &mut budget(),
                &|| false
            )
            .err(),
            Some(RecordsetError::Availability),
        );
    }
    for value in [Value::Int32(1), Value::Text("abc".into())] {
        assert_eq!(
            OwnedRecordset::new(
                schema(),
                vec![vec![StoredCell::Value(value), StoredCell::Uninspected]],
                RecordsetPresence::Assigned,
                &mut budget(),
                &|| false,
            )
            .err(),
            Some(RecordsetError::Schema(SchemaError::Row)),
        );
    }
    let required = Arc::new(
        RecordsetSchema::new(
            vec![Column::new("ID".into(), ScalarType::Int32, false).unwrap()],
            vec![MemberAvailability::Available],
            NameComparison::Exact,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    assert_eq!(
        OwnedRecordset::new(
            required,
            vec![vec![StoredCell::Value(Value::Null)]],
            RecordsetPresence::Assigned,
            &mut budget(),
            &|| false
        )
        .err(),
        Some(RecordsetError::Schema(SchemaError::Row)),
    );
}

#[test]
fn schema_validation_preserves_actual_name_policy_and_checks_empty_and_misaligned_metadata() {
    assert_eq!(
        RecordsetSchema::new(
            Vec::new(),
            Vec::new(),
            NameComparison::Exact,
            &mut budget(),
            &|| false
        )
        .err(),
        Some(RecordsetError::Schema(SchemaError::Width))
    );
    assert_eq!(
        RecordsetSchema::new(
            vec![Column::new("Name".into(), ScalarType::Text, true).unwrap()],
            Vec::new(),
            NameComparison::Exact,
            &mut budget(),
            &|| false,
        )
        .err(),
        Some(RecordsetError::Width)
    );
    for names in [NameComparison::Exact, NameComparison::AsciiInsensitive] {
        let result = RecordsetSchema::new(
            vec![
                Column::new("Name".into(), ScalarType::Text, true).unwrap(),
                Column::new("name".into(), ScalarType::Text, true).unwrap(),
            ],
            vec![MemberAvailability::Available; 2],
            names,
            &mut budget(),
            &|| false,
        );
        if names == NameComparison::Exact {
            assert_eq!(result.unwrap().names(), NameComparison::Exact);
        } else {
            assert_eq!(
                result.err(),
                Some(RecordsetError::Schema(SchemaError::Duplicate))
            );
        }
    }
}

#[test]
fn late_row_failure_and_caller_work_or_cancellation_never_publish_partial_records() {
    let rows = vec![
        vec![StoredCell::Value(Value::Null), StoredCell::Uninspected],
        vec![StoredCell::Value(Value::Null)],
    ];
    assert_eq!(
        OwnedRecordset::new(
            schema(),
            rows,
            RecordsetPresence::Assigned,
            &mut budget(),
            &|| false
        )
        .err(),
        Some(RecordsetError::Width)
    );
    let mut no_work = ResourceBudget::new(0, 0, 0);
    assert!(matches!(
        OwnedRecordset::new(
            schema(),
            Vec::new(),
            RecordsetPresence::Assigned,
            &mut no_work,
            &|| false
        ),
        Err(RecordsetError::Limit(_))
    ));
    assert_eq!(
        OwnedRecordset::new(
            schema(),
            Vec::new(),
            RecordsetPresence::Assigned,
            &mut budget(),
            &|| true
        )
        .err(),
        Some(RecordsetError::Cancelled)
    );
    let calls = std::cell::Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == 5
    };
    let rows = vec![
        vec![StoredCell::Value(Value::Null), StoredCell::Uninspected],
        vec![
            StoredCell::Value(Value::Text(String::new())),
            StoredCell::Uninspected,
        ],
    ];
    assert_eq!(
        OwnedRecordset::new(
            schema(),
            rows,
            RecordsetPresence::Assigned,
            &mut budget(),
            &cancelled
        )
        .err(),
        Some(RecordsetError::Cancelled)
    );
    assert_eq!(calls.get(), 5);
}

#[test]
fn schema_duplicate_traversal_is_cancellable_before_a_late_duplicate() {
    let columns = ["A", "B", "a"]
        .map(|name| Column::new(name.into(), ScalarType::Int32, false).unwrap())
        .into_iter()
        .collect();
    let calls = std::cell::Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == 6
    };
    assert_eq!(
        RecordsetSchema::new(
            columns,
            vec![MemberAvailability::Available; 3],
            NameComparison::AsciiInsensitive,
            &mut budget(),
            &cancelled,
        )
        .err(),
        Some(RecordsetError::Cancelled),
    );
    assert_eq!(calls.get(), 6);
}

#[test]
fn retained_footprints_include_spare_buffers_without_charging_shared_schema_twice() {
    fn declaration(name_capacity: usize) -> Arc<RecordsetSchema> {
        let mut name = String::with_capacity(name_capacity);
        name.push_str("Text");
        Arc::new(
            RecordsetSchema::new(
                vec![Column::new(name, ScalarType::Text, true).unwrap()],
                vec![MemberAvailability::Available],
                NameComparison::Exact,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        )
    }

    fn value(schema: Arc<RecordsetSchema>, text_capacity: usize) -> OwnedRecordset {
        let mut text = String::with_capacity(text_capacity);
        text.push_str("same");
        OwnedRecordset::new(
            schema,
            vec![vec![StoredCell::Value(Value::Text(text))]],
            RecordsetPresence::Assigned,
            &mut budget(),
            &|| false,
        )
        .unwrap()
    }

    let compact = declaration(4);
    let spare = declaration(4096);
    assert!(spare.retained_bytes() > compact.retained_bytes() + 4000);

    let compact_value = value(compact, 4);
    let shared_schema_value = value(Arc::clone(&spare), 4);
    assert_eq!(
        compact_value.retained_bytes(),
        shared_schema_value.retained_bytes()
    );

    let spare_value = value(spare, 8192);
    assert!(spare_value.retained_bytes() > shared_schema_value.retained_bytes() + 8000);
    assert_eq!(spare_value.rows(), shared_schema_value.rows());
}
