use super::joins::{join, source};
use super::*;
use tablecraft_access::NativeSource;
use tablecraft_core::values::Value;
use tablecraft_query::prepare;

#[test]
fn all_attributes_follow_keys_while_original_physical_fragments_remain_unchanged() {
    let low = &[0, 0, 0, 1];
    let high = &[0, 0, 0, 2];
    let rows = [
        Fragment {
            name1: Some("KeyA"),
            order: Some(high),
            ..column("a.Id")
        },
        Fragment {
            order: Some(high),
            ..source("A", Some("a"))
        },
        Fragment {
            expression: Some("a.Id"),
            order: Some(high),
            ..row(9)
        },
        Fragment {
            expression: Some("KeyA"),
            order: Some(high),
            ..row(11)
        },
        Fragment {
            name1: Some("P"),
            flag: Some(4),
            order: Some(high),
            ..row(2)
        },
        Fragment {
            name1: Some("KeyB"),
            order: Some(low),
            ..column("b.Id")
        },
        Fragment {
            order: Some(low),
            ..source("B", Some("b"))
        },
        Fragment {
            expression: Some("b.Id"),
            order: Some(low),
            ..row(9)
        },
        Fragment {
            expression: Some("KeyB"),
            name1: Some("D"),
            order: Some(low),
            ..row(11)
        },
        Fragment {
            name1: Some("Q"),
            flag: Some(4),
            order: Some(low),
            ..row(2)
        },
    ];
    let saved = compile(&rows).unwrap();
    let expected=parse_query("PARAMETERS Q LONG,P LONG; SELECT b.Id AS KeyB,a.Id AS KeyA FROM B AS b,A AS a GROUP BY b.Id,a.Id ORDER BY KeyB DESC,KeyA ASC",Default::default()).unwrap();
    assert_eq!(saved.query(), expected.query());
    assert_eq!(saved.definition().rows()[0].expression(), Some("a.Id"));
    assert_eq!(saved.definition().rows()[4].name1(), Some("P"));
    assert_eq!(saved.definition().rows()[9].name1(), Some("Q"));
}

#[test]
fn null_key_anywhere_preserves_all_storage_order_and_byte_keys_are_stable_unsigned_prefixes() {
    for (missing, sql) in [(false, "SELECT 1,2"), (true, "SELECT 2,1")] {
        let rows = [
            Fragment {
                order: if missing { None } else { Some(&[]) },
                ..row(0)
            },
            Fragment {
                order: Some(&[255]),
                ..column("2")
            },
            Fragment {
                order: Some(&[128]),
                ..column("1")
            },
        ];
        assert_eq!(
            compile(&rows).unwrap().query(),
            parse_query(sql, Default::default()).unwrap().query()
        );
    }
    let keys: [&[u8]; 7] = [&[128], &[0, 0], &[], &[0], &[128], &[127], &[255]];
    let expressions = ["1", "2", "3", "4", "5", "6", "7"];
    let rows: Vec<_> = keys
        .iter()
        .zip(expressions)
        .map(|(key, text)| Fragment {
            order: Some(key),
            ..column(text)
        })
        .collect();
    assert_eq!(
        compile(&rows).unwrap().query(),
        parse_query("SELECT 3,4,2,6,1,5,7", Default::default())
            .unwrap()
            .query()
    );
}

#[test]
fn sorted_native_outer_joins_and_late_predicates_execute_on_all_current_header_layouts() {
    let rows = [
        Fragment {
            name1: Some("KeyA"),
            ..column("a.Id")
        },
        Fragment {
            name1: Some("KeyB"),
            order: Some(&[0, 0, 0, 2]),
            ..column("b.Id")
        },
        Fragment {
            name1: Some("KeyC"),
            order: Some(&[0, 0, 0, 3]),
            ..column("c.Id")
        },
        source("Items", Some("a")),
        source("Items", Some("b")),
        source("Items", Some("c")),
        Fragment {
            order: Some(&[0, 0, 0, 3]),
            ..join(2, "b", "c", "b.Id=c.Id")
        },
        Fragment {
            order: Some(&[0, 0, 0, 2]),
            ..join(2, "b", "a", "b.Id<>2")
        },
        join(2, "a", "b", "a.Id=b.Id"),
    ];
    for code in [1, 2, 5, 6] {
        let mut bytes = database_with_query_flags(&rows, 0);
        bytes[20] = code;
        if code == 1 {
            bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        let before = bytes.clone();
        let mut source = NativeSource::new(
            Cursor::new(bytes.as_slice()),
            CatalogScope::new(7),
            budget(),
            &|| false,
        )
        .unwrap();
        let saved = source
            .read_saved_query("Saved", Default::default())
            .unwrap();
        assert_eq!(saved.definition().rows()[6].name1(), Some("b"));
        assert_eq!(
            prepare(saved.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .unwrap()
                .rows(),
            &[
                vec![Value::Int32(1), Value::Int32(1), Value::Int32(1)],
                vec![Value::Int32(2), Value::Null, Value::Null],
                vec![Value::Int32(3), Value::Int32(3), Value::Int32(3)],
            ]
        );
        drop(source);
        assert_eq!(bytes, before);
    }
}

#[test]
fn sorting_and_final_publication_check_every_cancellation_point_and_work_memory_limits() {
    let keys: Vec<_> = (0..32_u32).rev().map(u32::to_be_bytes).collect();
    let rows: Vec<_> = keys
        .iter()
        .map(|key| Fragment {
            order: Some(key),
            ..column("1")
        })
        .collect();
    let calls = Cell::new(0);
    reconstruct_saved_select(
        definition(&rows, 0),
        Default::default(),
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    for stop in 1..=calls.get() {
        let checks = Cell::new(0);
        assert_eq!(
            reconstruct_saved_select(
                definition(&rows, 0),
                Default::default(),
                &mut budget(),
                &|| {
                    checks.set(checks.get() + 1);
                    checks.get() == stop
                }
            )
            .err(),
            Some(E::Cancelled)
        );
    }
    for mut allowance in [
        ResourceBudget::new(256, 100, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 100, 100),
    ] {
        assert!(matches!(
            reconstruct_saved_select(
                definition(&rows, 0),
                Default::default(),
                &mut allowance,
                &|| false
            ),
            Err(E::Limit(_))
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn arbitrary_key_bytes_match_an_independent_stable_sort_or_whole_definition_null_policy(
        keys in prop::collection::vec(prop::collection::vec(any::<u8>(),0..5),1..24), missing in any::<bool>()
    ) {
        let expressions:Vec<_>=(1..=keys.len()).map(|i|i.to_string()).collect();
        let rows:Vec<_>=keys.iter().enumerate().map(|(i,key)|Fragment{order:if missing&&i==0{None}else{Some(key)},..column(&expressions[i])}).collect();
        let mut positions:Vec<_>=(0..keys.len()).collect();
        if !missing {positions.sort_by_key(|&i|&keys[i]);}
        let sql=format!("SELECT {}",positions.iter().map(|&i|expressions[i].as_str()).collect::<Vec<_>>().join(","));
        let expected=parse_query(&sql,Default::default()).unwrap();
        let saved=compile(&rows).unwrap();
        prop_assert_eq!(saved.query(),expected.query());
        for (i,fragment) in saved.definition().rows().iter().enumerate() {
            prop_assert_eq!(fragment.expression(),Some(expressions[i].as_str()));
        }
    }
}
