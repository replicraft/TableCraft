use super::*;
use std::{cmp::Ordering, collections::BTreeMap};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{ExecutionOptions, prepare, source::CollationResolver};
use tablecraft_sql::parse_query;

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

const ORDINAL: Ordinal = Ordinal;

#[test]
fn original_native_text_groups_cross_snapshot_limits_with_explicit_or_stored_policies() {
    let bytes = include_bytes!("../../../../../docs/examples/Access_Example_VBA.accdb");
    for stored_policy in [false, true] {
        let native = NativeSource::new_with_snapshot_limits(
            Cursor::new(bytes.to_vec()),
            SCOPE,
            budget(),
            &|| false,
            TableSnapshotLimits::new(1, 3).unwrap(),
        )
        .unwrap();
        let comparisons = RefCell::new(ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000));
        let (native, policy) = if stored_policy {
            let (native, policy) = native
                .with_database_collation_resolver(&comparisons)
                .unwrap();
            (native, Some(policy))
        } else {
            (native, None)
        };
        let collation = policy
            .as_ref()
            .map(|policy| policy.default_collation() as &dyn Collation)
            .unwrap_or(&ORDINAL);
        let resolver = policy
            .as_ref()
            .map(|policy| policy as &dyn CollationResolver);
        let syntax = parse_query(
            "SELECT Left(NBR_VALUE,1) AS Bucket,Count(*) AS N FROM ROULETTE_TB GROUP BY Left(NBR_VALUE,1)",
            Default::default(),
        )
        .unwrap();
        let plan = prepare(syntax.query(), &native).unwrap();
        for declared_integral in [false, true] {
            let mut actual = BTreeMap::new();
            let summary = plan
                .execute_progressive(
                    &native,
                    &[],
                    ExecutionOptions {
                        declared_integral,
                        collation: Some(collation),
                        collation_resolver: resolver,
                        ..Default::default()
                    },
                    &mut |row| {
                        let [Value::Text(key), Value::Int64(count)] = row else {
                            panic!("Text grouping changed its retained carrier or Count type");
                        };
                        assert!(actual.insert(key.clone(), *count).is_none());
                        Ok::<_, ()>(())
                    },
                )
                .unwrap();
            // The original American roulette table contains 0, 00 and 1..36.
            // Count first digits independently of its physical wheel order.
            let mut expected = BTreeMap::from([
                ("0".to_owned(), 2),
                ("1".to_owned(), 11),
                ("2".to_owned(), 11),
                ("3".to_owned(), 8),
            ]);
            for digit in '4'..='9' {
                expected.insert(digit.to_string(), 1);
            }
            assert_eq!(actual, expected);
            assert_eq!(summary.scanned_rows, 38);
            assert_eq!(summary.emitted_rows, 10);
        }
        assert_eq!(
            native.scan("ROULETTE_TB", &mut |_| Ok(())),
            Err(QueryError::StructureLimit)
        );
    }
}
