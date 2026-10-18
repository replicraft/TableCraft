use tablecraft_core::ids::{CursorBookmark, CursorId, RecordId, SourceGeneration, TableId};
use tablecraft_core::limits::LimitError;
use tablecraft_core::values::{ExpressionState, ExpressionValue, Value, ValueError};

#[test]
fn record_and_bookmark_identity_includes_its_owner_and_generation() {
    let table = TableId::from_u128(1);
    let generation = SourceGeneration::new(7);
    let record = RecordId::new(table, generation, vec![0, 255, 3]);
    assert_ne!(
        record,
        RecordId::new(TableId::from_u128(2), generation, vec![0, 255, 3])
    );
    assert_ne!(
        record,
        RecordId::new(table, SourceGeneration::new(8), vec![0, 255, 3])
    );
    assert_ne!(record, RecordId::new(table, generation, vec![0, 255, 4]));
    assert_eq!(record.table(), table);
    assert_eq!(record.generation(), generation);
    assert_eq!(record.token(), &[0, 255, 3]);

    let cursor = CursorId::from_u128(4);
    let bookmark = CursorBookmark::new(cursor, generation, vec![9]);
    assert_ne!(
        bookmark,
        CursorBookmark::new(CursorId::from_u128(5), generation, vec![9])
    );
    assert_ne!(
        bookmark,
        CursorBookmark::new(cursor, SourceGeneration::new(8), vec![9])
    );
    assert_eq!(bookmark.cursor(), cursor);
    assert_eq!(bookmark.generation(), generation);
    assert_eq!(bookmark.token(), &[9]);
}

#[test]
fn storage_distinguishes_null_empty_text_width_and_expression_states() {
    assert_ne!(Value::Null, Value::Text(String::new()));
    assert_ne!(Value::Byte(1), Value::Int16(1));
    assert_ne!(Value::Int16(1), Value::Int32(1));
    assert_ne!(Value::Int32(1), Value::Int64(1));
    assert_eq!(
        Value::try_from(ExpressionValue::Value(Value::Null)),
        Ok(Value::Null)
    );
    assert_eq!(
        Value::try_from(ExpressionValue::Value(Value::Text(String::new()))),
        Ok(Value::Text(String::new()))
    );
    for (expression, state) in [
        (ExpressionValue::Empty, ExpressionState::Empty),
        (ExpressionValue::Missing, ExpressionState::Missing),
        (ExpressionValue::Error(42), ExpressionState::Error),
    ] {
        assert_eq!(
            Value::try_from(expression),
            Err(ValueError::ExpressionNotStorable(state))
        );
    }
    for number in [i64::MIN, 9_007_199_254_740_993, i64::MAX] {
        let value = Value::Int64(number);
        let Value::Int64(actual) = value else {
            panic!("wrong carrier");
        };
        assert_eq!(actual, number);
    }
}

#[test]
fn diagnostics_do_not_contain_cell_contents_or_locator_tokens() {
    assert_eq!(
        format!("{:?}", Value::Text("password=private".into())),
        "Text(\"[redacted]\")"
    );
    assert_eq!(
        format!("{:?}", Value::Binary(b"password=private".to_vec())),
        "Binary(\"[redacted]\")"
    );
    assert_eq!(
        format!("{:?}", Value::Int64(123456789)),
        "Int64(\"[redacted]\")"
    );
    assert_eq!(
        format!("{:?}", ExpressionValue::Error(123456789)),
        "Error([redacted])"
    );
    let record = RecordId::new(
        TableId::from_u128(1),
        SourceGeneration::new(2),
        b"password=private".to_vec(),
    );
    let bookmark = CursorBookmark::new(
        CursorId::from_u128(1),
        SourceGeneration::new(2),
        b"password=private".to_vec(),
    );
    for debug in [format!("{record:?}"), format!("{bookmark:?}")] {
        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains("password"));
        assert!(!debug.contains("112, 97"));
    }
    fn requires_error<T: std::error::Error>() {}
    requires_error::<ValueError>();
    requires_error::<LimitError>();
}
