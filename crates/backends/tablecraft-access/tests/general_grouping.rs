//! Synthetic repeated-key stress; no application/sample record payload is embedded.
use std::cell::RefCell;
use tablecraft_access::NativeCollationResolver;
use tablecraft_ace::NativeSortOrder;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, Source, prepare,
    source::{CollationResolver, MemorySource, MemoryTable, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

const GENERAL_POLICY: TextCollationId = TextCollationId::from_host(1033);

struct Rows(MemorySource);

impl Source for Rows {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.0.columns(table)
    }

    fn text_collations(&self, _: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(&[Some(GENERAL_POLICY)])
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.0.scan(table, visitor)
    }
}

struct WithoutHints<'a>(&'a dyn CollationResolver);

impl CollationResolver for WithoutHints<'_> {
    fn resolve(&self, policy: TextCollationId) -> Result<&dyn Collation, QueryError> {
        self.0.resolve(policy)
    }
}

#[test]
fn repeated_two_character_groups_fit_same_cumulative_budget_without_resident_memory_redefinition() {
    const GROUPS: usize = 249;
    const ROWS: usize = 100_000;
    const BYTES: u64 = 256 * 1024 * 1024;
    const STEPS: u64 = 100_000_000;
    let values = (0..ROWS)
        .map(|index| {
            let key = index % GROUPS;
            vec![Value::Text(format!(
                "{}{}",
                char::from(b'A' + (key / 26) as u8),
                char::from(b'A' + (key % 26) as u8)
            ))]
        })
        .collect();
    let source = Rows(
        MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![Column::new("K".into(), ScalarType::Text, false).unwrap()],
                values,
            )
            .unwrap(),
        ])
        .unwrap(),
    );
    let parsed = parse_query(
        "SELECT K,Count(*) FROM T GROUP BY K",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let execute = |resolver: &dyn CollationResolver| {
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation_resolver: Some(resolver),
                limits: QueryLimits {
                    bytes: BYTES,
                    rows: 1_000_000,
                    steps: STEPS,
                },
                ..Default::default()
            },
        )
    };
    let comparisons = RefCell::new(ResourceBudget::new(BYTES, 0, STEPS));
    let resolver = NativeCollationResolver::new(
        NativeSortOrder::from_native_bytes([9, 4, 0, 0]),
        &comparisons,
        &|| false,
    )
    .unwrap();
    let result = execute(&resolver).unwrap();
    assert_eq!(result.rows().len(), GROUPS);
    for (index, row) in result.rows().iter().enumerate() {
        assert_eq!(
            row[1],
            Value::Int64((ROWS / GROUPS + usize::from(index < ROWS % GROUPS)) as i64)
        );
    }
    let comparisons = RefCell::new(ResourceBudget::new(BYTES, 0, STEPS));
    let resolver = NativeCollationResolver::new(
        NativeSortOrder::from_native_bytes([9, 4, 0, 0]),
        &comparisons,
        &|| false,
    )
    .unwrap();
    let error = execute(&WithoutHints(&resolver))
        .err()
        .expect("strict linear comparisons exhaust their cumulative allowance");
    assert!(
        error.to_string().contains("Bytes resource limit exceeded"),
        "{error}"
    );
}

#[test]
fn singleton_unmapped_unicode_and_overlong_hint_inputs_keep_existing_group_semantics() {
    for value in [
        String::new(),
        "\0\u{80}\u{ffff}\u{fd6f}".into(),
        "🙂雪  ".into(),
        "x".repeat(256),
        "🙂".repeat(256),
    ] {
        let source = Rows(
            MemorySource::new(vec![
                MemoryTable::new(
                    "T".into(),
                    vec![Column::new("K".into(), ScalarType::Text, false).unwrap()],
                    vec![vec![Value::Text(value.clone())]],
                )
                .unwrap(),
            ])
            .unwrap(),
        );
        let parsed = parse_query(
            "SELECT K,Count(*) FROM T GROUP BY K",
            ParseLimits::default(),
        )
        .unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let budget = RefCell::new(ResourceBudget::new(1_000_000, 0, 1_000_000));
        let resolver = NativeCollationResolver::new(
            NativeSortOrder::from_native_bytes([9, 4, 0, 0]),
            &budget,
            &|| false,
        )
        .unwrap();
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), &[vec![Value::Text(value), Value::Int64(1)]]);
    }
}
