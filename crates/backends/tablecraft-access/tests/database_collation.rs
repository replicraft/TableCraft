//! Original metadata mutations over the registered inert fixture, never native qualification.
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    io::Cursor,
};
use tablecraft_access::NativeSource;
use tablecraft_ace::{InspectError, NativeSortOrder, compare_general_text, inspect_table_schema};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16384, 1_000_000)
}
fn copy() -> Vec<u8> {
    include_bytes!("../../../../docs/examples/Access_Example_VBA.accdb").to_vec()
}
fn orders(bytes: &mut [u8], database: [u8; 4], column: [u8; 4]) {
    // Independent registered-layout mutation, not an alternate production parser.
    for (i, mask) in [1, 27, 152, 253].into_iter().enumerate() {
        bytes[110 + i] = database[i] ^ mask;
    }
    let schema = inspect_table_schema(
        &mut Cursor::new(&*bytes),
        CatalogScope::new(1),
        "ROULETTE_TB",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let page = schema.definition_page() as usize * 4096;
    let indexes = u32::from_le_bytes(bytes[page + 51..page + 55].try_into().unwrap()) as usize;
    let offset = page + 63 + indexes * 12 + 25 + 11;
    bytes[offset..offset + 4].copy_from_slice(&column);
}
fn open_source(bytes: Vec<u8>) -> NativeSource<'static, Cursor<Vec<u8>>> {
    NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
        false
    })
    .unwrap()
}
#[test]
fn stored_general_families_select_full_policy_and_reuse_metadata_across_queries() {
    for version in [0, 1] {
        let mut bytes = copy();
        let raw = [9, 4, 0, version];
        orders(&mut bytes, raw, raw);
        let (source, policy) = open_source(bytes)
            .with_database_collation(budget())
            .unwrap();
        for (left, right) in [
            ("a", "A "),
            ("é", "e\u{301}"),
            ("\u{fd6f}", ""),
            ("ss", "ß"),
        ] {
            assert_eq!(
                policy.compare(left, right).unwrap(),
                compare_general_text(
                    NativeSortOrder::from_native_bytes(raw),
                    left,
                    right,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
            );
        }
        for sql in [
            "SELECT TOP 1 ORDR_NBR FROM ROULETTE_TB WHERE 'a'='A' ORDER BY ORDR_NBR",
            "SELECT Count(*) AS N FROM ROULETTE_TB WHERE NBR_VALUE LIKE '2*'",
        ] {
            let parsed = parse_query(sql, ParseLimits::default()).unwrap();
            let plan = prepare(parsed.query(), &source).unwrap();
            let result = plan
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        collation: Some(&policy),
                        like_mode: Some(tablecraft_expressions::predicates::LikeMode::Ansi89),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(
                result.rows(),
                if sql.contains("TOP") {
                    vec![vec![Value::Int32(1)]]
                } else {
                    vec![vec![Value::Int64(11)]]
                }
            );
        }
        // Another table's legacy text cannot silently inherit a 2010 database policy.
        assert_eq!(source.columns("TAG_GRP_TB").is_ok(), version == 0);
    }
}
#[test]
fn unknown_database_and_mixed_column_descriptors_never_fall_back() {
    for raw in [[0, 0, 0, 0], [9, 4, 1, 1], [9, 4, 0, 2], [31, 4, 0, 1]] {
        let mut bytes = copy();
        orders(&mut bytes, raw, [9, 4, 0, 0]);
        assert!(matches!(
            open_source(bytes).with_database_collation(budget()),
            Err(InspectError::Unsupported)
        ));
    }
    for raw in [[0, 0, 0, 0], [9, 4, 0, 1], [9, 4, 1, 0], [31, 4, 0, 0]] {
        let mut bytes = copy();
        orders(&mut bytes, [9, 4, 0, 0], raw);
        let (source, _) = open_source(bytes)
            .with_database_collation(budget())
            .unwrap();
        // Even an unprojected Text column is checked, including empty-result queries.
        for sql in [
            "SELECT ORDR_NBR FROM ROULETTE_TB WHERE FALSE",
            "SELECT Count(*) FROM ROULETTE_TB",
        ] {
            let parsed = parse_query(sql, ParseLimits::default()).unwrap();
            assert_eq!(
                prepare(parsed.query(), &source).err(),
                Some(QueryError::Unsupported)
            );
        }
        assert_eq!(
            source.scan("ROULETTE_TB", &mut |_| panic!("mixed policy leaked a row")),
            Err(QueryError::Unsupported)
        );
    }
}
#[test]
fn explicit_family_overrides_remain_independent_and_selection_precedes_schema_loading() {
    let source = open_source(copy());
    source.columns("ROULETTE_TB").unwrap();
    assert!(matches!(
        source.with_database_collation(budget()),
        Err(InspectError::Unsupported)
    ));
    let mut bytes = copy();
    orders(&mut bytes, [31, 4, 0, 1], [9, 4, 1, 0]);
    let source = open_source(bytes);
    assert_eq!(source.columns("ROULETTE_TB").unwrap().len(), 3);
    let explicit = tablecraft_access::GeneralCollation::new(
        NativeSortOrder::from_native_bytes([9, 4, 0, 1]),
        budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(explicit.compare("a", "A").unwrap(), Ordering::Equal);
}
#[test]
fn collections_cannot_bypass_descriptor_resolution_in_either_builder_order() {
    for mode in 0..3 {
        let original = open_source(copy());
        let configured = match mode {
            0 => original.with_multivalues(),
            1 => original.with_collections(),
            _ => original.with_decoded_attachments(),
        };
        assert!(matches!(
            configured.with_database_collation(budget()),
            Err(InspectError::Unsupported)
        ));
        let (original, _) = open_source(copy())
            .with_database_collation(budget())
            .unwrap();
        let configured = match mode {
            0 => original.with_multivalues(),
            1 => original.with_collections(),
            _ => original.with_decoded_attachments(),
        };
        assert_eq!(
            configured.multivalue_columns("ROULETTE_TB").err(),
            Some(QueryError::Unsupported)
        );
        if mode != 0 {
            assert_eq!(
                configured.collection_members("ROULETTE_TB").err(),
                Some(QueryError::Unsupported)
            );
        }
    }
}
#[test]
fn cancellation_and_comparison_exhaustion_use_original_shared_controls() {
    let cancelled = Cell::new(false);
    let callback = || cancelled.get();
    let original = NativeSource::new(
        Cursor::new(copy()),
        CatalogScope::new(1),
        budget(),
        &callback,
    )
    .unwrap();
    cancelled.set(true);
    assert!(matches!(
        original.with_database_collation(budget()),
        Err(InspectError::Cancelled)
    ));
    cancelled.set(false);
    let original = NativeSource::new(
        Cursor::new(copy()),
        CatalogScope::new(1),
        budget(),
        &callback,
    )
    .unwrap();
    let (source, policy) = original.with_database_collation(budget()).unwrap();
    source.columns("ROULETTE_TB").unwrap();
    cancelled.set(true);
    assert_eq!(policy.compare("a", "A"), Err(EvalError::Cancelled));
    assert_eq!(
        source.columns("ROULETTE_TB").err(),
        Some(QueryError::Cancelled)
    );
    assert!(matches!(
        open_source(copy()).with_database_collation(ResourceBudget::new(0, 0, 0)),
        Err(InspectError::Limit(_))
    ));
    let (_, policy) = open_source(copy())
        .with_database_collation(ResourceBudget::new(512, 0, 10000))
        .unwrap();
    let mut successes = 0;
    loop {
        match policy.compare("a", "A") {
            Ok(_) => successes += 1,
            Err(EvalError::Limit(_)) => break,
            other => panic!("{other:?}"),
        };
        assert!(successes < 100);
    }
    assert!(successes > 0);
}

struct GuardedReader {
    cursor: Cursor<Vec<u8>>,
    forbidden: Vec<std::ops::Range<u64>>,
    attempts: std::rc::Rc<Cell<usize>>,
}
impl tablecraft_ace::NativeReadSource for GuardedReader {}
impl std::io::Read for GuardedReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let start = self.cursor.position();
        let end = start.saturating_add(buffer.len() as u64);
        if self
            .forbidden
            .iter()
            .any(|r| start < r.end && end > r.start)
        {
            self.attempts.set(self.attempts.get() + 1);
            return Err(std::io::Error::other("user record pages forbidden"));
        }
        std::io::Read::read(&mut self.cursor, buffer)
    }
}
impl std::io::Seek for GuardedReader {
    fn seek(&mut self, position: std::io::SeekFrom) -> std::io::Result<u64> {
        std::io::Seek::seek(&mut self.cursor, position)
    }
}
#[test]
fn selection_and_binding_read_no_user_records_and_mismatches_fail_before_scan() {
    for version in [0, 1] {
        let mut bytes = copy();
        orders(&mut bytes, [9, 4, 0, 0], [9, 4, 0, version]);
        let schema = inspect_table_schema(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        // Independent registered page header geometry; block all owned data pages.
        let forbidden = bytes
            .as_chunks::<4096>()
            .0
            .iter()
            .enumerate()
            .filter_map(|(i, page)| {
                (page[..2] == [1, 1]
                    && u32::from_le_bytes(page[4..8].try_into().unwrap())
                        == schema.definition_page())
                .then_some(i as u64 * 4096..(i as u64 + 1) * 4096)
            })
            .collect::<Vec<_>>();
        assert!(!forbidden.is_empty());
        let attempts = std::rc::Rc::new(Cell::new(0));
        let source = NativeSource::new(
            GuardedReader {
                cursor: Cursor::new(bytes),
                forbidden,
                attempts: attempts.clone(),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap();
        let (source, _policy) = source.with_database_collation(budget()).unwrap();
        let parsed = parse_query(
            "SELECT ORDR_NBR FROM ROULETTE_TB WHERE FALSE",
            ParseLimits::default(),
        )
        .unwrap();
        let plan = prepare(parsed.query(), &source);
        assert_eq!(attempts.get(), 0);
        if version == 0 {
            assert!(plan.is_ok());
            // A direct scan demonstrates that the guard covers real record access.
            assert!(
                source
                    .scan("ROULETTE_TB", &mut |_| panic!("guarded rows leaked"))
                    .is_err()
            );
            assert!(attempts.get() > 0);
        } else {
            assert_eq!(plan.err(), Some(QueryError::Unsupported));
            assert_eq!(attempts.get(), 0);
        }
    }
}

#[test]
fn each_observed_policy_selection_checkpoint_can_cancel_before_publication() {
    let visits = Cell::new(0usize);
    let stop = Cell::new(None);
    let callback = || {
        let current = visits.get() + 1;
        visits.set(current);
        stop.get() == Some(current)
    };
    let original = NativeSource::new(
        Cursor::new(copy()),
        CatalogScope::new(1),
        budget(),
        &callback,
    )
    .unwrap();
    visits.set(0);
    let _ = original.with_database_collation(budget()).unwrap();
    let count = visits.get();
    assert!(count > 1);
    for point in 1..=count {
        stop.set(None);
        let original = NativeSource::new(
            Cursor::new(copy()),
            CatalogScope::new(1),
            budget(),
            &callback,
        )
        .unwrap();
        visits.set(0);
        stop.set(Some(point));
        assert!(
            matches!(
                original.with_database_collation(budget()),
                Err(InspectError::Cancelled)
            ),
            "checkpoint {point}"
        );
    }
}

#[test]
fn native_text_policy_ids_preserve_exact_unknown_descriptors_without_activating_them() {
    for raw in [
        [9, 4, 0, 0],
        [9, 4, 0, 1],
        [0, 0, 0, 0],
        [9, 4, 1, 1],
        [31, 4, 0, 2],
    ] {
        let mut bytes = copy();
        orders(&mut bytes, [9, 4, 0, 0], raw);
        let source = open_source(bytes);
        let policies = source.text_collations("ROULETTE_TB").unwrap();
        assert_eq!(policies.len(), 3);
        assert!(policies[0].is_none());
        assert!(policies[2].is_none());
        // Independent expected descriptor bytes; query consumers only see a host token.
        assert_eq!(
            policies[1].unwrap().host_key(),
            u64::from(u32::from_le_bytes(raw))
        );
        let query =
            parse_query("SELECT NBR_VALUE FROM ROULETTE_TB", ParseLimits::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        assert_eq!(
            plan.execute(&source, &[], ExecutionOptions::default())
                .unwrap()
                .rows()
                .len(),
            38
        );
    }
}

#[test]
fn per_field_selection_keeps_native_defaults_and_unknown_projection_separate() {
    for default in [0, 1] {
        let mut bytes = copy();
        orders(&mut bytes, [9, 4, 0, default], [9, 4, 0, 1 - default]);
        let pool = RefCell::new(budget());
        let (source, resolver) = open_source(bytes)
            .with_database_collation_resolver(&pool)
            .unwrap();
        let parsed=parse_query("SELECT TOP 1 ORDR_NBR,NBR_VALUE='2' AS FieldValue,ChrW(128)<ChrW(769) AS DefaultValue FROM ROULETTE_TB ORDER BY ORDR_NBR",ParseLimits::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(resolver.default_collation()),
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![
                Value::Int32(1),
                Value::Boolean(true),
                Value::Boolean(default == 0)
            ]]
        );
    }
    let mut bytes = copy();
    orders(&mut bytes, [9, 4, 0, 0], [31, 4, 0, 1]);
    let pool = RefCell::new(budget());
    let (source, resolver) = open_source(bytes)
        .with_database_collation_resolver(&pool)
        .unwrap();
    let query = parse_query("SELECT NBR_VALUE FROM ROULETTE_TB", ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows()
            .len(),
        38
    );
    let query = parse_query(
        "SELECT NBR_VALUE='2' FROM ROULETTE_TB",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        prepare(query.query(), &source).unwrap().execute(
            &source,
            &[],
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                ..Default::default()
            }
        ),
        Err(QueryError::Evaluation(EvalError::CollationRequired))
    ));
}
#[test]
fn per_field_builder_keeps_collection_guards_and_separate_io_allowances() {
    for mode in 0..3 {
        let pool = RefCell::new(budget());
        let original = open_source(copy());
        let configured = match mode {
            0 => original.with_multivalues(),
            1 => original.with_collections(),
            _ => original.with_decoded_attachments(),
        };
        assert!(matches!(
            configured.with_database_collation_resolver(&pool),
            Err(InspectError::Unsupported)
        ));
        let (original, _) = open_source(copy())
            .with_database_collation_resolver(&pool)
            .unwrap();
        let configured = match mode {
            0 => original.with_multivalues(),
            1 => original.with_collections(),
            _ => original.with_decoded_attachments(),
        };
        assert_eq!(
            configured.multivalue_columns("ROULETTE_TB").err(),
            Some(QueryError::Unsupported)
        );
    }
    let pool = RefCell::new(ResourceBudget::new(1000, 0, 1_000_000));
    let (source, resolver) = open_source(copy())
        .with_database_collation_resolver(&pool)
        .unwrap();
    for _ in 0..100 {
        if resolver.default_collation().compare("ss", "ß").is_err() {
            break;
        }
    }
    assert!(matches!(
        resolver.default_collation().compare("ss", "ß"),
        Err(EvalError::Limit(_))
    ));
    let query = parse_query("SELECT Count(*) FROM ROULETTE_TB", ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
        [vec![Value::Int64(38)]]
    );
}

#[test]
fn per_field_configuration_and_binding_remain_metadata_only_with_mixed_orders() {
    let mut bytes = copy();
    orders(&mut bytes, [9, 4, 0, 0], [9, 4, 0, 1]);
    let schema = inspect_table_schema(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "ROULETTE_TB",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let forbidden = bytes
        .as_chunks::<4096>()
        .0
        .iter()
        .enumerate()
        .filter_map(|(i, page)| {
            (page[..2] == [1, 1]
                && u32::from_le_bytes(page[4..8].try_into().unwrap()) == schema.definition_page())
            .then_some(i as u64 * 4096..(i as u64 + 1) * 4096)
        })
        .collect::<Vec<_>>();
    assert!(!forbidden.is_empty());
    let attempts = std::rc::Rc::new(Cell::new(0));
    let source = NativeSource::new(
        GuardedReader {
            cursor: Cursor::new(bytes),
            forbidden,
            attempts: attempts.clone(),
        },
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap();
    let pool = RefCell::new(budget());
    let (source, _) = source.with_database_collation_resolver(&pool).unwrap();
    let query = parse_query(
        "SELECT NBR_VALUE='2' FROM ROULETTE_TB WHERE FALSE",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(prepare(query.query(), &source).is_ok());
    assert_eq!(attempts.get(), 0);
    assert!(
        source
            .scan("ROULETTE_TB", &mut |_| panic!("guarded data leaked"))
            .is_err()
    );
    assert!(attempts.get() > 0);
}
