use super::joins::{join, source};
use super::*;
use tablecraft_access::NativeSource;
use tablecraft_core::values::Value;
use tablecraft_query::prepare;

#[test]
fn stored_delimiters_normalize_once_for_sources_aliases_columns_and_parameters() {
    let rows = [
        Fragment {
            name1: Some("[Enter a minimum]"),
            flag: Some(4),
            ..row(2)
        },
        source("[Order Items]", Some("[Select]")),
        Fragment {
            name1: Some("[Chosen Value]"),
            ..column("[Select].[ID]+[Enter a minimum]")
        },
    ];
    let saved = compile(&rows).unwrap();
    assert_eq!(saved.query(), parse_query("PARAMETERS [Enter a minimum] LONG; SELECT [Select].ID+[Enter a minimum] AS [Chosen Value] FROM [Order Items] AS [Select]",Default::default()).unwrap().query());
    assert_eq!(
        saved.definition().rows()[0].name1(),
        Some("[Enter a minimum]")
    );
    assert_eq!(saved.definition().rows()[1].name1(), Some("[Order Items]"));
    assert_eq!(saved.definition().rows()[1].name2(), Some("[Select]"));
    assert_eq!(saved.definition().rows()[2].name1(), Some("[Chosen Value]"));
    let derived = Fragment {
        expression: Some("SELECT 1 AS N"),
        name2: Some("[q]"),
        ..row(5)
    };
    assert_eq!(
        compile(&[column("q.N"), derived]).unwrap().query(),
        parse_query("SELECT q.N FROM (SELECT 1 AS N) AS q", Default::default())
            .unwrap()
            .query()
    );
}

#[test]
fn quoted_join_endpoints_match_raw_labels_while_duplicate_checks_use_logical_names() {
    for (left, right, left_label, right_label, sql) in [
        (
            source("[Left Items]", None),
            source("[Right Items]", None),
            "[Left Items]",
            "[Right Items]",
            "SELECT 1 FROM [Left Items] LEFT JOIN [Right Items] ON True",
        ),
        (
            source("[Left Items]", Some("[a]")),
            source("Right Items", Some("[b]")),
            "[a]",
            "[b]",
            "SELECT 1 FROM [Left Items] AS a LEFT JOIN [Right Items] AS b ON True",
        ),
    ] {
        let saved = compile(&[
            column("1"),
            left,
            right,
            join(2, left_label, right_label, "True"),
        ])
        .unwrap();
        assert_eq!(
            saved.query(),
            parse_query(sql, Default::default()).unwrap().query()
        );
    }
    for rows in [
        vec![
            column("1"),
            source("Items", Some("a")),
            source("Other", Some("[A]")),
        ],
        vec![column("1"), source("Items", None), source("[items]", None)],
        vec![
            column("1"),
            Fragment {
                name1: Some("P"),
                flag: Some(4),
                ..row(2)
            },
            Fragment {
                name1: Some("[p]"),
                flag: Some(4),
                order: Some(&[0, 0, 0, 2]),
                ..row(2)
            },
        ],
    ] {
        assert_eq!(compile(&rows).err(), Some(E::InvalidMetadata));
    }
    // The reference matches native join labels literally. A missing delimiter
    // is not authority to synthesize or silently repair a stored endpoint.
    assert_eq!(
        compile(&[
            column("1"),
            source("L", Some("[a]")),
            source("R", Some("b")),
            join(1, "a", "b", "True")
        ])
        .err(),
        Some(E::InvalidMetadata)
    );
}

#[test]
fn malformed_delimiters_and_object_punctuation_cannot_escape_their_name_role() {
    for name in [
        "[]",
        "[[P]]",
        "[P",
        "P]",
        "[P].[Q]",
        "P.Q",
        "[P!Q]",
        "[P`Q]",
        "[P\nQ]",
        "[P] WHERE True --",
        "[P]] AS [Injected]",
    ] {
        for rows in [
            vec![column("1"), source(name, None)],
            vec![column("1"), source("Items", Some(name))],
            vec![Fragment {
                name1: Some(name),
                ..column("1")
            }],
        ] {
            assert!(compile(&rows).is_err(), "{name}");
        }
        let parameter = Fragment {
            name1: Some(name),
            flag: Some(4),
            ..row(2)
        };
        if ["P.Q", "[P!Q]", "[P`Q]"].contains(&name) {
            assert!(compile(&[column("1"), parameter]).is_ok());
        } else {
            assert!(compile(&[column("1"), parameter]).is_err(), "{name}");
        }
    }
}

#[test]
fn native_quoted_sources_and_parameters_execute_without_changing_fragments_or_database_bytes() {
    let rows = [
        Fragment {
            name1: Some("[Floor]"),
            flag: Some(4),
            ..row(2)
        },
        source("[Items]", Some("[q]")),
        Fragment {
            name1: Some("[Chosen Key]"),
            ..column("q.Id")
        },
        Fragment {
            expression: Some("q.Id>=Floor"),
            ..row(8)
        },
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
        assert_eq!(
            prepare(saved.query(), &source)
                .unwrap()
                .execute(&source, &[("Floor", Value::Int32(2))], Default::default())
                .unwrap()
                .rows(),
            &[vec![Value::Int32(2)], vec![Value::Int32(3)]]
        );
        drop(source);
        assert_eq!(bytes, before);
    }
}

proptest! {
    #[test]
    fn bracketed_valid_symbol_names_match_independently_parsed_aliases(name in "[a-zA-Z0-9_ #+%()/;-]{1,40}") {
        let stored=format!("[{name}]");
        let saved=compile(&[Fragment{name1:Some(&stored),..column("1")}]).unwrap();
        let sql=format!("SELECT 1 AS [{name}]");
        let expected=parse_query(&sql,Default::default()).unwrap();
        prop_assert_eq!(saved.query(),expected.query());
    }
}
