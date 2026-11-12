//! UPDATE uses the same declared and runtime text lineage as retrieval.
use super::{Editor, Recorded, budget};
use std::{cell::Cell, cmp::Ordering, rc::Rc};
use tablecraft_core::values::Value;
use tablecraft_db::Snapshot;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{
    MutationOptions, QueryError, UpdateError, plan_batch_update_with_options,
    source::{CollationResolver, TextCollationId},
};
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaPolicy};
use tablecraft_sql::{ParseLimits, parse_query};

const FOLD: TextCollationId = TextCollationId::from_host(11);
const ORDINAL: TextCollationId = TextCollationId::from_host(12);

pub(super) struct Comparer(bool);

impl Collation for Comparer {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        if self.0 {
            Ok(left
                .bytes()
                .map(|byte| byte.to_ascii_lowercase())
                .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase())))
        } else {
            Ok(left.cmp(right))
        }
    }
}

static FOLD_COMPARER: Comparer = Comparer(true);
pub(super) static DEFAULT_COMPARER: Comparer = Comparer(false);

#[derive(Default)]
struct Resolver {
    calls: Cell<usize>,
    cancel_on_resolve: bool,
    cancelled: Cell<bool>,
}

impl CollationResolver for Resolver {
    fn resolve(&self, policy: TextCollationId) -> Result<&dyn Collation, QueryError> {
        self.calls.set(self.calls.get() + 1);
        if self.cancel_on_resolve {
            self.cancelled.set(true);
        }
        match policy {
            FOLD => Ok(&FOLD_COMPARER),
            ORDINAL => Ok(&DEFAULT_COMPARER),
            _ => Err(EvalError::CollationRequired.into()),
        }
    }
}

fn editor() -> (Editor, Rc<Cell<usize>>) {
    let columns = vec![
        Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
        Column::new("Name".into(), ScalarType::Text, false).unwrap(),
        Column::new("Other".into(), ScalarType::Text, false).unwrap(),
        Column::new("Score".into(), ScalarType::Int32, false).unwrap(),
    ];
    let rows = ["Alpha", "Beta"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            vec![
                Value::Int32(index as i32 + 1),
                Value::Text(name.into()),
                Value::Text(name.to_ascii_lowercase()),
                Value::Int32(0),
            ]
        })
        .collect();
    let snapshot = Snapshot::new(
        "Items".into(),
        columns,
        rows,
        SchemaPolicy {
            max_name_bytes: 255,
            max_columns: 256,
            names: NameComparison::AsciiInsensitive,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let calls = Rc::new(Cell::new(0));
    (
        Editor {
            snapshot,
            calls: Rc::clone(&calls),
            backend_steps: 0,
        },
        calls,
    )
}

#[test]
fn field_resolver_governs_where_and_eager_set_lineage_instead_of_database_default() {
    let (editor, calls) = editor();
    let resolver = Resolver::default();
    let policies = [None, Some(FOLD), Some(ORDINAL), None];
    let query = parse_query(
        "UPDATE Items SET Score=IIf(IIf(True,Name,'literal')='ALPHA',77,0) WHERE Name='alpha'",
        ParseLimits::default(),
    )
    .unwrap();
    let result = plan_batch_update_with_options(
        editor,
        query.query(),
        &mut budget(),
        &mut budget(),
        MutationOptions {
            like_mode: None,
            parameters: None,
            collation: Some(&DEFAULT_COMPARER),
            collation_resolver: Some(&resolver),
            text_collations: &policies,
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(result.matched_rows(), 1);
    let expected: Recorded = vec![("Score".into(), vec![(0, Value::Int32(77))])];
    assert_eq!(result.into_backend(), expected);
    assert_eq!(calls.get(), 1);
    assert!(
        resolver.calls.get() >= 3,
        "WHERE and SET resolve actual field policy"
    );
}

#[test]
fn typed_text_parameters_preserve_the_stored_field_policy_in_where_and_set() {
    let (editor, calls) = editor();
    let resolver = Resolver::default();
    let policies = [None, Some(FOLD), Some(ORDINAL), None];
    let query = parse_query(
        "PARAMETERS needle TEXT; UPDATE Items SET Score=IIf(needle=Name,77,0) WHERE Name=needle",
        ParseLimits::default(),
    )
    .unwrap();
    let parameters = [("needle", Value::Text("ALPHA".into()))];

    let result = plan_batch_update_with_options(
        editor,
        query.query(),
        &mut budget(),
        &mut budget(),
        MutationOptions {
            like_mode: None,
            parameters: Some(&parameters),
            collation: Some(&DEFAULT_COMPARER),
            collation_resolver: Some(&resolver),
            text_collations: &policies,
        },
        &|| false,
    )
    .unwrap();

    assert_eq!(result.matched_rows(), 1);
    let expected: Recorded = vec![("Score".into(), vec![(0, Value::Int32(77))])];
    assert_eq!(result.into_backend(), expected);
    assert_eq!(calls.get(), 1);
    assert!(
        resolver.calls.get() >= 3,
        "both comparison orders retain the field's policy instead of the default"
    );
}

#[test]
fn conflicting_or_unknown_field_policies_refuse_before_the_consuming_backend_finish() {
    for (statement, policies, expected) in [
        (
            "UPDATE Items SET Score=IIf(IIf(True,Name,Other)='ALPHA',77,0) WHERE Id=1",
            [None, Some(FOLD), Some(ORDINAL), None],
            QueryError::CollationConflict,
        ),
        (
            "UPDATE Items SET Score=IIf(Name=Other,77,0) WHERE Id=1",
            [None, Some(FOLD), Some(ORDINAL), None],
            QueryError::CollationConflict,
        ),
        (
            "UPDATE Items SET Score=77 WHERE Name='alpha'",
            [None, Some(TextCollationId::from_host(99)), None, None],
            QueryError::Evaluation(EvalError::CollationRequired),
        ),
    ] {
        let (editor, calls) = editor();
        let resolver = Resolver::default();
        let query = parse_query(statement, ParseLimits::default()).unwrap();
        let result = plan_batch_update_with_options(
            editor,
            query.query(),
            &mut budget(),
            &mut budget(),
            MutationOptions {
                like_mode: None,
                parameters: None,
                collation: Some(&DEFAULT_COMPARER),
                collation_resolver: Some(&resolver),
                text_collations: &policies,
            },
            &|| false,
        );
        let Err(UpdateError::Query(actual)) = result else {
            panic!("unresolved or conflicting field policy must refuse atomically");
        };
        assert_eq!(actual, expected);
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn malformed_policy_alignment_and_provider_cancellation_cannot_finish_a_batch() {
    for policies in [vec![Some(FOLD)], vec![Some(FOLD), None, None, None]] {
        let (editor, calls) = editor();
        let query = parse_query("UPDATE Items SET Score=77", ParseLimits::default()).unwrap();
        let result = plan_batch_update_with_options(
            editor,
            query.query(),
            &mut budget(),
            &mut budget(),
            MutationOptions {
                parameters: None,
                text_collations: &policies,
                ..Default::default()
            },
            &|| false,
        );
        assert!(matches!(
            result,
            Err(UpdateError::Query(QueryError::Source))
        ));
        assert_eq!(calls.get(), 0);
    }

    let (editor, calls) = editor();
    let resolver = Resolver {
        cancel_on_resolve: true,
        ..Default::default()
    };
    let policies = [None, Some(FOLD), None, None];
    let query = parse_query(
        "UPDATE Items SET Score=77 WHERE Name='alpha'",
        ParseLimits::default(),
    )
    .unwrap();
    let result = plan_batch_update_with_options(
        editor,
        query.query(),
        &mut budget(),
        &mut budget(),
        MutationOptions {
            like_mode: None,
            parameters: None,
            collation: Some(&DEFAULT_COMPARER),
            collation_resolver: Some(&resolver),
            text_collations: &policies,
        },
        &|| resolver.cancelled.get(),
    );
    assert!(matches!(
        result,
        Err(UpdateError::Query(QueryError::Cancelled))
    ));
    assert_eq!(calls.get(), 0);
}

mod like;
