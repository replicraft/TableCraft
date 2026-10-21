use super::joins::source;
use super::*;
use tablecraft_access::NativeSource;
use tablecraft_core::values::Value;
use tablecraft_query::{prepare, source::MemorySource};

fn parameter(name: &str) -> Fragment<'_> {
    Fragment {
        name1: Some(name),
        flag: Some(4),
        ..row(2)
    }
}

#[test]
fn raw_and_bracketed_prompt_names_preserve_literal_meaning_and_original_metadata() {
    for (stored, name) in [
        ("Enter No. of items", "Enter No. of items"),
        ("[Enter No. of items]", "Enter No. of items"),
        ("P.Q", "P.Q"),
        ("[P.Q]", "P.Q"),
        ("Floor!", "Floor!"),
        ("[Ask!Now]", "Ask!Now"),
        ("Ask! now", "Ask! now"),
        ("Rate `value`", "Rate `value`"),
        ("P) OR (True; --", "P) OR (True; --"),
    ] {
        let expression = format!("[{name}]");
        let saved = compile(&[parameter(stored), column(&expression)]).unwrap();
        let expected = parse_query(
            &format!("PARAMETERS [{name}] LONG; SELECT [{name}]"),
            Default::default(),
        )
        .unwrap();
        assert_eq!(saved.query(), expected.query());
        assert_eq!(saved.definition().rows()[0].name1(), Some(stored));
        let source = MemorySource::new(vec![]).unwrap();
        assert_eq!(
            prepare(saved.query(), &source)
                .unwrap()
                .execute(&source, &[(name, Value::Int32(7))], Default::default())
                .unwrap()
                .rows(),
            &[vec![Value::Int32(7)]]
        );
    }
}

#[test]
fn native_punctuated_prompts_filter_rows_on_all_current_header_layouts_without_writes() {
    let rows = [
        parameter("Enter No. of items"),
        source("Items", None),
        column("Id"),
        Fragment {
            expression: Some("Id>=[Enter No. of items]"),
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
                .execute(
                    &source,
                    &[("Enter No. of items", Value::Int32(2))],
                    Default::default()
                )
                .unwrap()
                .rows(),
            &[vec![Value::Int32(2)], vec![Value::Int32(3)]]
        );
        drop(source);
        assert_eq!(bytes, before);
    }
}

#[test]
fn qualified_control_names_malformed_delimiters_unicode_and_normalized_duplicates_remain_errors() {
    for name in [
        "Forms!P",
        "Ask!Now",
        "Forms!My Form!Code",
        "Forms![My Form]!Code",
        "[P].[Q]",
        "[[P]]",
        "P]",
        "[P",
        "[]",
        "P\nQ",
        "P\0Q",
        "é. nombre",
    ] {
        assert_eq!(
            compile(&[parameter(name), column("1")]).err(),
            Some(E::Unsupported),
            "{name}"
        );
    }
    let rows = [
        parameter("P.Q"),
        Fragment {
            order: Some(&[0, 0, 0, 2]),
            ..parameter("[p.q]")
        },
        column("1"),
    ];
    assert_eq!(compile(&rows).err(), Some(E::InvalidMetadata));
}

#[test]
fn prompt_guards_retain_cumulative_limits_and_all_publication_cancellation_checkpoints() {
    let rows = [
        parameter("Enter No. of items"),
        column("[Enter No. of items]+1"),
    ];
    let checks = Cell::new(0);
    reconstruct_saved_select(
        definition(&rows, 0),
        Default::default(),
        &mut budget(),
        &|| {
            checks.set(checks.get() + 1);
            false
        },
    )
    .unwrap();
    for stop in 1..=checks.get() {
        let calls = Cell::new(0);
        assert_eq!(
            reconstruct_saved_select(
                definition(&rows, 0),
                Default::default(),
                &mut budget(),
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }
            )
            .err(),
            Some(E::Cancelled)
        );
    }
    assert!(matches!(
        reconstruct_saved_select(
            definition(&rows, 0),
            ParseLimits {
                max_bytes: 48,
                ..Default::default()
            },
            &mut budget(),
            &|| false
        ),
        Err(E::SyntaxLimit)
    ));
    assert!(matches!(
        reconstruct_saved_select(
            definition(&rows, 0),
            Default::default(),
            &mut ResourceBudget::new(1024, 100, 1_000_000),
            &|| false
        ),
        Err(E::Limit(_))
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn stored_prompt_symbols_match_an_independent_declaration_and_expression(name in "P[a-zA-Z0-9 .`?;:/()#%_+-]{0,40}") {
        let expression=format!("[{name}]");
        let expected=parse_query(&format!("PARAMETERS [{name}] LONG; SELECT [{name}]"),Default::default()).unwrap();
        let saved=compile(&[parameter(&name),column(&expression)]).unwrap();
        prop_assert_eq!(saved.query(),expected.query());
    }
}
