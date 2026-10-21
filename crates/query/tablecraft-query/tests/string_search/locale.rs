//! Host routing model, independent of any native locale implementation.
use super::*;
use std::{cell::RefCell, cmp::Ordering, ops::Range};
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, LocaleId, StringSearch},
};

struct Host(RefCell<Vec<u32>>);
impl Collation for Host {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        panic!("explicit LCID used a database comparer")
    }
    fn find_locale(
        &self,
        haystack: &str,
        needle: &str,
        search: StringSearch,
        locale: LocaleId,
    ) -> Result<Option<Range<usize>>, EvalError> {
        self.0.borrow_mut().push(locale.raw());
        if ![1033, 0x10407].contains(&locale.raw()) {
            return Err(EvalError::CollationRequired);
        }
        let StringSearch::ForwardFrom(start) = search else {
            panic!("InStr issued reverse search")
        };
        let hay = haystack
            .to_ascii_lowercase()
            .encode_utf16()
            .collect::<Vec<_>>();
        let needle = needle
            .to_ascii_lowercase()
            .encode_utf16()
            .collect::<Vec<_>>();
        Ok(hay
            .windows(needle.len())
            .enumerate()
            .find(|(i, value)| *i >= start && *value == needle)
            .map(|(i, _)| i..i + needle.len()))
    }
}
fn options(host: &Host) -> ExecutionOptions<'_> {
    ExecutionOptions {
        collation: Some(host),
        ..Default::default()
    }
}
#[test]
fn lcid_parameters_rebind_and_correlated_grouped_reads_keep_explicit_provider() {
    let source = source();
    let host = Host(RefCell::new(vec![]));
    let parsed = parse_query(
        "PARAMETERS L LONG; SELECT InStr(1,Label,'AN',L) FROM Words",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert!(host.0.borrow().is_empty());
    for id in [1033, 0x10407] {
        assert_eq!(
            plan.execute(&source, &[("L", Value::Int32(id))], options(&host))
                .unwrap()
                .rows(),
            [
                vec![Value::Int32(2)],
                vec![Value::Int32(2)],
                vec![Value::Null],
                vec![Value::Int32(4)]
            ]
        );
        assert!(
            host.0
                .borrow()
                .iter()
                .rev()
                .take(3)
                .all(|seen| *seen == id as u32)
        );
    }
    let parsed=parse_query("SELECT (SELECT InStr(1,W.Label,'AN',1033)) FROM Words AS W WHERE InStr(1,W.Label,'AN',1033)>2",ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options(&host))
            .unwrap()
            .rows(),
        [vec![Value::Int32(4)]]
    );
    let parsed=parse_query("SELECT InStr(1,Label,'AN',1033) AS P,Count(*) FROM Words GROUP BY InStr(1,Label,'AN',1033) ORDER BY P",ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options(&host))
            .unwrap()
            .rows(),
        [
            vec![Value::Null, Value::Int64(1)],
            vec![Value::Int32(2), Value::Int64(2)],
            vec![Value::Int32(4), Value::Int64(1)]
        ]
    );
}
#[test]
fn lcid_errors_preserve_eager_evaluation_and_return_no_partial_result() {
    let source = source();
    let host = Host(RefCell::new(vec![]));
    for (sql, error) in [
        (
            "SELECT InStr(1,Label,'AN',1055) FROM Words",
            EvalError::CollationRequired,
        ),
        ("SELECT InStr(1,NULL,'AN',1/0)", EvalError::DivisionByZero),
        ("SELECT InStr(1,NULL,'AN',1048576)", EvalError::Domain),
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], options(&host))
                .err(),
            Some(error.into())
        );
    }
    let parsed = parse_query(
        "SELECT InStr(1,'banana','AN',1033) UNION ALL SELECT InStr(1,'banana','AN',1055)",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options(&host))
            .err(),
        Some(EvalError::CollationRequired.into())
    );
}
