use super::{raw_support::RawResolver, support::*};
use std::{cmp::Ordering, ops::Range};
use tablecraft_expressions::EvalError;
use tablecraft_expressions::comparison::{
    Collation, StringCompareMode, StringSearch, StringSearchProfile,
};
use tablecraft_query::source::MemorySource;

struct Search;

impl Collation for Search {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        panic!("explicit search must not activate default equality")
    }

    fn string_search_profile(&self, mode: StringCompareMode) -> StringSearchProfile {
        if mode == StringCompareMode::Database {
            StringSearchProfile::Utf16
        } else {
            StringSearchProfile::Unicode
        }
    }

    fn find_utf16(
        &self,
        haystack: &[u16],
        needle: &[u16],
        search: StringSearch,
        _: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        let matches = haystack
            .windows(needle.len())
            .enumerate()
            .filter(|(start, candidate)| {
                *candidate == needle
                    && match search {
                        StringSearch::ForwardFrom(first) => *start >= first,
                        StringSearch::ReverseThrough(last) => *start + needle.len() <= last,
                    }
            })
            .map(|(start, _)| start..start + needle.len())
            .collect::<Vec<_>>();
        Ok(match search {
            StringSearch::ForwardFrom(_) => matches.first().cloned(),
            StringSearch::ReverseThrough(_) => matches.last().cloned(),
        })
    }
}

#[test]
fn raw_search_uses_default_provider_while_sql_predicates_keep_field_policies() {
    let source = Fixture::new();
    let parsed = parse_query(
        "SELECT InStr(1,A&ChrW(55296),'A',2),(A&ChrW(55296))=('A'&ChrW(55296)),InStr(1,U&ChrW(55296),'x',2),InStr(1,DLookup('ChrW(55296)','T','Id=1'),ChrW(55296),2) FROM T WHERE Id=1",
        Default::default(),
    ).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Search),
                collation_resolver: Some(&RawResolver),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![
            Value::Int32(0),
            Value::Boolean(true),
            Value::Int32(0),
            Value::Int32(1)
        ]]
    );
}

#[test]
fn raw_provider_search_composes_with_parameters_derived_rows_and_extrema() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT InStr(1,S,ChrW(55296),2),InStrRev(S,ChrW(55296),-1,2) FROM (SELECT ChrW(P)&ChrW(55296) AS S) AS Q",
        Default::default(),
    ).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let options = ExecutionOptions {
        collation: Some(&Search),
        ..Default::default()
    };
    for (code, first) in [(65, 2), (55296, 1), (57343, 2)] {
        let result = plan
            .execute(&source, &[("P", Value::Int32(code))], options)
            .unwrap();
        assert_eq!(result.rows(), &[vec![Value::Int32(first), Value::Int32(2)]]);
    }
    let parsed = parse_query(
        "SELECT InStr(1,Min(S),ChrW(55296),2) FROM (SELECT ChrW(55296) AS S) AS Q",
        Default::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], options)
        .unwrap();
    assert_eq!(result.rows(), &[vec![Value::Int32(1)]]);
}
