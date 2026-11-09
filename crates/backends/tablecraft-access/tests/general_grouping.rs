//! Synthetic repeated-key stress; no application/sample record payload is embedded.
use std::cell::RefCell;
use tablecraft_access::NativeCollationResolver;
use tablecraft_ace::NativeSortOrder;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, Source, prepare,
    source::{CollationResolver, MemorySource, MemoryTable, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

const GENERAL_POLICY: TextCollationId = TextCollationId::from_host(1033);

struct Rows {
    source: MemorySource,
    policy: [Option<TextCollationId>; 1],
}

impl Rows {
    fn new(source: MemorySource) -> Self {
        Self {
            source,
            policy: [Some(GENERAL_POLICY)],
        }
    }
}

impl Source for Rows {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.source.columns(table)
    }

    fn text_collations(&self, _: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(&self.policy)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.source.scan(table, visitor)
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
    let source = Rows::new(
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
        let source = Rows::new(
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

#[test]
fn occupied_comparison_pool_declines_optional_hint_but_preserves_actual_comparison_failure() {
    for count in [1, 2] {
        let source = Rows::new(
            MemorySource::new(vec![
                MemoryTable::new(
                    "T".into(),
                    vec![Column::new("K".into(), ScalarType::Text, false).unwrap()],
                    vec![vec![Value::Text("A".into())]; count],
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
        let comparisons = RefCell::new(ResourceBudget::new(1_000_000, 0, 1_000_000));
        let resolver = NativeCollationResolver::new(
            NativeSortOrder::from_native_bytes([9, 4, 0, 0]),
            &comparisons,
            &|| false,
        )
        .unwrap();

        let held = comparisons.borrow();
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        );

        if count == 1 {
            assert_eq!(
                result.unwrap().rows(),
                &[vec![Value::Text("A".into()), Value::Int64(1)]]
            );
        } else {
            assert!(matches!(
                result,
                Err(QueryError::Evaluation(EvalError::CollationFailed))
            ));
        }

        drop(held);
    }
}

#[test]
fn native_general_hints_preserve_full_comparison_groups_for_both_sort_versions() {
    let shared_primary = "a".repeat(80);
    let values = [
        "apple".into(),
        "APPLE".into(),
        "apple   ".into(),
        "e".into(),
        "é".into(),
        "É".into(),
        "æ".into(),
        "ae".into(),
        "a\u{1}b".into(),
        "ab".into(),
        format!("{shared_primary}b"),
        format!("{shared_primary}c"),
    ];

    for version in [0_u8, 1] {
        let source = Rows {
            source: MemorySource::new(vec![
                MemoryTable::new(
                    "T".into(),
                    vec![Column::new("K".into(), ScalarType::Text, false).unwrap()],
                    values
                        .iter()
                        .map(|value| vec![Value::Text(value.clone())])
                        .collect(),
                )
                .unwrap(),
            ])
            .unwrap(),
            policy: [Some(TextCollationId::from_host(
                1033 | (u64::from(version) << 24),
            ))],
        };
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
                    ..Default::default()
                },
            )
            .unwrap()
        };
        let comparisons = RefCell::new(ResourceBudget::new(10_000_000, 0, 10_000_000));
        let resolver = NativeCollationResolver::new(
            NativeSortOrder::from_native_bytes([9, 4, 0, version]),
            &comparisons,
            &|| false,
        )
        .unwrap();

        let hinted = execute(&resolver);
        let linear = execute(&WithoutHints(&resolver));
        assert_eq!(hinted.rows(), linear.rows(), "sort version {version}");
        assert!(hinted.rows().len() < values.len());
        assert_eq!(
            hinted
                .rows()
                .iter()
                .filter(|row| {
                    matches!(&row[0], Value::Text(value) if value.starts_with(&shared_primary))
                })
                .count(),
            2,
            "truncated primary prefixes must still compare their full text"
        );
    }
}
