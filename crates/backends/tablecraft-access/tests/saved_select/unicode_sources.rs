use super::joins::{join, source};
use super::*;
use tablecraft_access::{NativeSource, reconstruct_saved_read};
use tablecraft_core::values::Value;
use tablecraft_query::{ExecutionOptions, prepare};

#[test]
fn unicode_named_and_derived_sources_retain_exact_labels_and_owned_names() {
    for (left, right) in [
        ("左側", "右側"),
        ("Ordres 🦀", "Éléments"),
        ("قائمة", "פריטים"),
        ("e\u{301}", "é"),
    ] {
        let condition = format!("[{left}].ID=[{right}].ID");
        let expected =
            format!("SELECT 1 FROM A AS [{left}] LEFT JOIN B AS [{right}] ON {condition}");
        let rows = [
            column("1"),
            source("A", Some(left)),
            source("B", Some(right)),
            join(2, left, right, &condition),
        ];
        assert_eq!(
            compile(&rows).unwrap().query(),
            parse_query(&expected, Default::default()).unwrap().query()
        );
        let rows = [
            column("1"),
            source(left, None),
            source(right, None),
            join(2, left, right, "True"),
        ];
        let expected = format!("SELECT 1 FROM [{left}] LEFT JOIN [{right}] ON True");
        assert_eq!(
            compile(&rows).unwrap().query(),
            parse_query(&expected, Default::default()).unwrap().query()
        );
        let derived = Fragment {
            name2: Some(left),
            expression: Some("SELECT ID FROM A"),
            ..row(5)
        };
        let rows = [
            column("1"),
            derived,
            source("B", Some(right)),
            join(1, left, right, &condition),
        ];
        let expected = format!(
            "SELECT 1 FROM (SELECT ID FROM A) AS [{left}] INNER JOIN B AS [{right}] ON {condition}"
        );
        assert_eq!(
            compile(&rows).unwrap().query(),
            parse_query(&expected, Default::default()).unwrap().query()
        );
    }
}

#[test]
fn unicode_labels_keep_raw_delimiters_ascii_case_and_duplicate_checks() {
    assert!(
        compile(&[
            column("1"),
            source("A", Some("[左a]")),
            source("B", Some("右b")),
            join(1, "[左A]", "右B", "True")
        ])
        .is_ok()
    );
    for (left, right) in [("左a", "[左A]"), ("🦀", "🦀"), ("é", "[é]")] {
        assert_eq!(
            compile(&[
                column("1"),
                source("A", Some(left)),
                source("B", Some(right))
            ])
            .err(),
            Some(E::InvalidMetadata)
        );
    }
    for endpoint in [
        "左a",
        "[左á]",
        "[左a] WHERE False --",
        "[左a]\n",
        "[左a].[ID]",
    ] {
        assert_eq!(
            compile(&[
                column("1"),
                source("A", Some("[左a]")),
                source("B", Some("右")),
                join(1, endpoint, "右", "True")
            ])
            .err(),
            Some(E::InvalidMetadata)
        );
    }
    // No normalization or Unicode case folding is inferred for native labels.
    for (label, endpoint) in [("é", "e\u{301}"), ("É", "é"), ("Σ", "σ")] {
        assert_eq!(
            compile(&[
                column("1"),
                source("A", Some(label)),
                source("B", Some("右")),
                join(1, endpoint, "右", "True")
            ])
            .err(),
            Some(E::InvalidMetadata)
        );
    }
}

#[test]
fn unicode_saved_joins_execute_and_retain_original_fragments_across_priority_headers() {
    let rows = [
        source("Items", Some("[左🦀]")),
        source("Items", Some("右")),
        Fragment {
            name1: Some("左の値"),
            ..column("[左🦀].Id")
        },
        Fragment {
            name1: Some("右の値"),
            order: Some(&[0, 0, 0, 2]),
            ..column("右.Id")
        },
        join(2, "[左🦀]", "右", "[左🦀].Id+1=右.Id"),
        Fragment {
            expression: Some("[左🦀].Id"),
            name1: Some("A"),
            ..row(11)
        },
    ];
    for code in [1, 2, 5, 6] {
        let mut bytes = database_with_query_flags(&rows, 0);
        bytes[20] = code;
        if code == 1 {
            bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        let original = bytes.clone();
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
        assert_eq!(saved.definition().rows()[0].name2(), Some("[左🦀]"));
        let result = prepare(saved.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        assert_eq!(result.columns(), &["左の値", "右の値"]);
        assert_eq!(
            result.rows(),
            &[
                vec![Value::Int32(1), Value::Int32(2)],
                vec![Value::Int32(2), Value::Int32(3)],
                vec![Value::Int32(3), Value::Null],
            ]
        );
        let named = parse_query("SELECT * FROM Saved", Default::default()).unwrap();
        assert_eq!(
            prepare(named.query(), &source)
                .unwrap()
                .execute(&source, &[], ExecutionOptions::default())
                .unwrap()
                .rows(),
            result.rows()
        );
        assert_eq!(bytes, original);
    }
}

#[test]
fn unicode_forest_construction_checks_cancellation_and_cumulative_byte_work_limits() {
    let rows = [
        column("1"),
        source("A", Some("左🦀")),
        source("B", Some("右é")),
        join(1, "左🦀", "右é", "True"),
    ];
    let calls = Cell::new(0);
    let stop = Cell::new(usize::MAX);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == stop.get()
    };
    let execute = || {
        reconstruct_saved_select(
            definition(&rows, 0),
            Default::default(),
            &mut budget(),
            &cancelled,
        )
    };
    execute().unwrap();
    for checkpoint in 1..=calls.get() {
        calls.set(0);
        stop.set(checkpoint);
        assert_eq!(execute().err(), Some(E::Cancelled));
    }
    for mut budget in [
        ResourceBudget::new(1, 16_384, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 16_384, 1),
    ] {
        assert!(matches!(
            reconstruct_saved_select(
                definition(&rows, 0),
                Default::default(),
                &mut budget,
                &|| false
            ),
            Err(E::Limit(_))
        ));
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x554e49434f44454a),
        ..Default::default()
    })]
    #[test]
    fn multibyte_join_labels_match_independently_parsed_forests(
        symbols in prop::collection::vec(prop::sample::select(vec!['左', '右', '🦀', 'é', 'Σ', 'ش', 'א', '\u{301}']), 1..12),
        outer in any::<bool>(),
    ) {
        let symbols = symbols.into_iter().collect::<String>();
        let left = format!("左a{symbols}");
        let right = format!("右b{symbols}");
        let predicate = format!("[{left}].ID=[{right}].ID");
        let rows = [column("1"), source("A", Some(&left)), source("B", Some(&right)), join(if outer { 2 } else { 1 }, &left, &right, &predicate)];
        let expected = format!("SELECT 1 FROM A AS [{left}] {} JOIN B AS [{right}] ON {predicate}", if outer { "LEFT" } else { "INNER" });
        let parsed = parse_query(&expected, Default::default()).unwrap();
        let actual = compile(&rows).unwrap();
        prop_assert_eq!(actual.query(), parsed.query());
    }
}

#[test]
fn crosstab_reconstruction_shares_unicode_join_labels_without_new_authority() {
    let rows = [
        Fragment {
            flag: Some(6),
            ..row(1)
        },
        source("Items", Some("左")),
        source("Items", Some("右")),
        join(1, "左", "右", "左.Id=右.Id"),
        Fragment {
            flag: Some(2),
            name1: Some("Bucket"),
            ..column("0")
        },
        Fragment {
            flag: Some(0),
            order: Some(&[0, 0, 0, 2]),
            ..column("Count(*)")
        },
        Fragment {
            flag: Some(1),
            order: Some(&[0, 0, 0, 3]),
            ..column("右.Id")
        },
        Fragment {
            flag: Some(2),
            expression: Some("0"),
            ..row(9)
        },
        Fragment {
            flag: Some(1),
            expression: Some("右.Id"),
            order: Some(&[0, 0, 0, 2]),
            ..row(9)
        },
    ];
    let saved = reconstruct_saved_read(
        definition(&rows, 0x10),
        Default::default(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let expected = parse_query("TRANSFORM Count(*) SELECT 0 AS Bucket FROM Items AS 左 INNER JOIN Items AS 右 ON 左.Id=右.Id GROUP BY 0 PIVOT 右.Id", Default::default()).unwrap();
    assert_eq!(saved.query(), expected.query());
    let bytes = database_with_query_flags(&rows, 0x10);
    let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(7), budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(
        prepare(saved.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        &[vec![
            Value::Int64(0),
            Value::Int64(1),
            Value::Int64(1),
            Value::Int64(1)
        ]]
    );
}
