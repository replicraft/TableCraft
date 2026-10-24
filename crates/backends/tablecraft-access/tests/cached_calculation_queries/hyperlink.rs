use super::{
    CatalogScope, Cell, Cursor, ExecutionOptions, NativeSource, ParseLimits, Rc, Value, Watching,
    budget, execute, memo_fixture, parse_query, prepare,
};

const FLAGS: usize = 15;

fn database(corrupt: bool) -> Vec<u8> {
    let text = "Display#https://example.invalid/a#part#é🦀";
    let encoded = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut last = memo_fixture::wrapped(b"l\0a\0t\0e\0");
    if corrupt {
        last[0] = 1;
    }
    let mut bytes = memo_fixture::database(
        5,
        false,
        &[
            Some(memo_fixture::inline(&memo_fixture::wrapped(&encoded))),
            None,
            Some(memo_fixture::inline(&last)),
        ],
        &[],
    );
    bytes[memo_fixture::FIELD_START + FLAGS] = 0x82;
    bytes
}

#[test]
fn hyperlink_caches_remain_explicit_row_free_at_binding_and_preserve_raw_text() {
    let bytes = database(false);
    let ordinary = NativeSource::new(
        Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap();
    assert!(execute(&ordinary, "SELECT Score FROM Items").is_err());
    let forbidden = Rc::new(Cell::new(true));
    let reads = Rc::new(Cell::new(0));
    let source = NativeSource::new(
        Watching {
            data: Cursor::new(bytes),
            forbid: forbidden.clone(),
            reads: reads.clone(),
        },
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_stored_calculations()
    .unwrap();
    let parsed = parse_query("SELECT Id,Score FROM Items", ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        source.is_stored_calculation("Items", 1).unwrap(),
        Some(true)
    );
    assert_eq!(reads.get(), 0);
    forbidden.set(false);
    let result = plan
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(
        result.rows()[0],
        [
            Value::Int32(1),
            Value::Text("Display#https://example.invalid/a#part#é🦀".into()),
        ]
    );
    assert_eq!(result.rows()[1], [Value::Int32(2), Value::Null]);
    forbidden.set(true);
    assert_eq!(
        execute(&source, "SELECT Count(Score) FROM Items").unwrap(),
        [vec![Value::Int64(2)]]
    );
}

#[test]
fn corrupt_hyperlink_cache_is_pruned_or_refused_without_partial_cache_publication() {
    let source = NativeSource::new(
        Cursor::new(database(true)),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap()
    .with_stored_calculations()
    .unwrap();
    assert_eq!(
        execute(&source, "SELECT Count(*) FROM Items").unwrap(),
        [vec![Value::Int64(3)]]
    );
    for query in [
        "SELECT TOP 1 Score FROM Items",
        "SELECT Score FROM Items WHERE Id=-1",
    ] {
        assert!(execute(&source, query).is_err());
    }
}
