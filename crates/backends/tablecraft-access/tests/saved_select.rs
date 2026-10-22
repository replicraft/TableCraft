#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
#[allow(dead_code)]
mod base;
#[path = "saved_select/crosstabs.rs"]
mod crosstabs;
#[path = "saved_select/derived.rs"]
mod derived;
#[path = "saved_select/domains.rs"]
mod domains;
#[path = "saved_select/execution.rs"]
mod execution;
#[path = "../../../engines/tablecraft-ace/tests/saved_queries/fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "saved_select/forests.rs"]
mod forests;
#[path = "saved_select/identifiers.rs"]
mod identifiers;
#[path = "saved_select/joins.rs"]
mod joins;
#[path = "saved_select/ordering.rs"]
mod ordering;
#[path = "saved_select/parameter_prompts.rs"]
mod parameter_prompts;
#[path = "saved_select/parameters.rs"]
mod parameters;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;
#[path = "saved_select/unions.rs"]
mod unions;

use fixture::{Fragment, QUERY_ID, database_with_query_flags};
use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_access::{SavedSelect, SavedSelectError as E, reconstruct_saved_select};
use tablecraft_ace::{SavedQueryDefinition, read_saved_query_definition};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16_384, 1_000_000)
}
fn definition(fragments: &[Fragment<'_>], flags: u32) -> SavedQueryDefinition {
    read_saved_query_definition(
        &mut Cursor::new(database_with_query_flags(fragments, flags)),
        CatalogScope::new(7),
        "Saved",
        &mut budget(),
        &|| false,
    )
    .unwrap_or_else(|error| {
        panic!(
            "fixture read {error:?}; flags {flags:x}; attributes {:?}",
            fragments
                .iter()
                .map(|row| row.attribute)
                .collect::<Vec<_>>()
        )
    })
}
fn compile(fragments: &[Fragment<'_>]) -> Result<SavedSelect, E> {
    reconstruct_saved_select(
        definition(fragments, 0x80000000),
        ParseLimits::default(),
        &mut budget(),
        &|| false,
    )
}
fn row(attribute: u8) -> Fragment<'static> {
    Fragment {
        object: Some(QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    }
}
fn column(expression: &str) -> Fragment<'_> {
    Fragment {
        expression: Some(expression),
        ..row(6)
    }
}

#[test]
fn local_select_preserves_flags_clauses_aliases_order_and_native_identity_without_execution() {
    let mut flags = row(3);
    flags.flag = Some(1 | 4 | 8 | 16 | 32);
    flags.name1 = Some("25");
    let mut table = row(5);
    table.name1 = Some("Order Items");
    table.name2 = Some("src");
    let mut first = column("[src].[ID]");
    first.name1 = Some("Result ID");
    let mut second = column("Count(*)");
    second.name1 = Some("Total");
    second.order = Some(&[0, 0, 0, 2]);
    let mut filter = row(8);
    filter.expression = Some("NeverRunMe([src].[ID]) > 0");
    let mut group = row(9);
    group.expression = Some("[src].[ID]");
    let mut having = row(10);
    having.expression = Some("Count(*) > 1");
    let mut order = row(11);
    order.expression = Some("[Total]");
    order.name1 = Some("d");
    let mut order2 = row(11);
    order2.expression = Some("[Result ID]");
    order2.name1 = Some("A");
    order2.order = Some(&[0, 0, 0, 2]);
    let fragments = [
        row(0),
        flags,
        table,
        first,
        second,
        filter,
        group,
        having,
        order,
        order2,
        row(255),
    ];
    for code in [5, 6] {
        let mut bytes = database_with_query_flags(&fragments, 0x80000000);
        bytes[20] = code;
        let definition = read_saved_query_definition(
            &mut Cursor::new(&bytes),
            CatalogScope::new(7),
            "Saved",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let result =
            reconstruct_saved_select(definition, ParseLimits::default(), &mut budget(), &|| false)
                .unwrap();
        let expected = parse_query("SELECT DISTINCTROW TOP 25 PERCENT [src].[ID] AS [Result ID], Count(*) AS [Total], * FROM [Order Items] AS [src] WHERE NeverRunMe([src].[ID]) > 0 GROUP BY [src].[ID] HAVING Count(*) > 1 ORDER BY [Total] DESC, [Result ID] ASC WITH OWNERACCESS OPTION", Default::default()).unwrap();
        assert_eq!(result.query(), expected.query());
        assert_eq!(result.definition().origin().native_id(), QUERY_ID);
        assert_eq!(result.definition().origin().flags(), 0x80000000);
        assert_eq!(result.definition().rows().len(), 11);
        assert_eq!(
            result.definition().rows()[3].expression(),
            Some("[src].[ID]")
        );
        assert_eq!(database_with_query_flags(&fragments, 0x80000000), {
            bytes[20] = 6;
            bytes
        });
    }
}

#[test]
fn select_flags_are_distinct_and_invalid_counts_or_conflicts_do_not_become_defaults() {
    for (bits, count, sql) in [
        (0, None, "SELECT 1"),
        (2, None, "SELECT DISTINCT 1"),
        (8, None, "SELECT DISTINCTROW 1"),
        (16, Some("0"), "SELECT TOP 0 1"),
        (18, Some("4294967295"), "SELECT DISTINCT TOP 4294967295 1"),
        (24, Some("12"), "SELECT DISTINCTROW TOP 12 1"),
        (48, Some("100"), "SELECT TOP 100 PERCENT 1"),
        (4, None, "SELECT 1 WITH OWNERACCESS OPTION"),
    ] {
        let mut flags = row(3);
        flags.flag = Some(bits);
        flags.name1 = count;
        assert_eq!(
            compile(&[flags, column("1")]).unwrap().query(),
            parse_query(sql, Default::default()).unwrap().query()
        );
    }
    for (bits, count, expected) in [
        (64, None, E::Unsupported),
        (-1, None, E::Unsupported),
        (10, None, E::InvalidMetadata),
        (16, None, E::InvalidMetadata),
        (16, Some(""), E::InvalidMetadata),
        (16, Some("-1"), E::InvalidMetadata),
        (16, Some("1 FROM Secret"), E::InvalidMetadata),
        (16, Some("4294967296"), E::InvalidMetadata),
        (48, Some("101"), E::InvalidMetadata),
        (32, None, E::InvalidMetadata),
        (0, Some("1"), E::InvalidMetadata),
    ] {
        let mut flags = row(3);
        flags.flag = Some(bits);
        flags.name1 = count;
        assert_eq!(compile(&[flags, column("1")]).err(), Some(expected));
    }
}

#[test]
fn metadata_profile_rejects_unhandled_fields_actions_ambiguous_clauses_and_order_keys() {
    for attribute in [4, 12, 200] {
        assert_eq!(
            compile(&[column("1"), row(attribute)]).err(),
            Some(E::Unsupported)
        );
    }
    let mut unknown = row(6);
    unknown.attribute = None;
    assert_eq!(compile(&[unknown]).err(), Some(E::Unsupported));
    for flags in [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0xf0] {
        assert_eq!(
            reconstruct_saved_select(
                definition(&[column("1")], flags),
                Default::default(),
                &mut budget(),
                &|| false
            )
            .err(),
            Some(E::Unsupported)
        );
    }
    assert_eq!(compile(&[]).err(), Some(E::InvalidMetadata));
    assert_eq!(compile(&[row(0), row(255)]).err(), Some(E::InvalidMetadata));
    for attribute in [0, 1, 3, 8, 10, 255] {
        let mut first = row(attribute);
        let mut second = row(attribute);
        if attribute == 1 {
            first.flag = Some(1);
            second.flag = Some(1);
        }
        if [8, 10].contains(&attribute) {
            first.expression = Some("True");
            second.expression = Some("True");
        }
        assert_eq!(
            compile(&[column("1"), first, second]).err(),
            Some(E::InvalidMetadata)
        );
    }
    for kind in 0..4 {
        let mut bad = column("1");
        match kind {
            0 => bad.extra = Some(0),
            1 => bad.flag = Some(1),
            2 => bad.name2 = Some("unused"),
            _ => bad.expression = None,
        }
        assert!(compile(&[bad]).is_err());
    }
    for (key, sql) in [
        (&[0, 0, 0, 1][..], "SELECT 1,2"),
        (&[0, 0, 0, 0][..], "SELECT 2,1"),
    ] {
        let mut second = column("2");
        second.order = Some(key);
        assert_eq!(
            compile(&[column("1"), second]).unwrap().query(),
            parse_query(sql, Default::default()).unwrap().query()
        );
    }
    let mut bad_type = row(1);
    bad_type.flag = Some(3);
    assert_eq!(
        compile(&[column("1"), bad_type]).err(),
        Some(E::InvalidMetadata)
    );
    let mut external = row(5);
    external.name1 = Some("Items");
    external.expression = Some("external.accdb");
    assert_eq!(
        compile(&[column("1"), external]).err(),
        Some(E::Unsupported)
    );
    let mut bad_order = row(11);
    bad_order.expression = Some("1");
    bad_order.name1 = Some("descending");
    assert_eq!(
        compile(&[column("1"), bad_order]).err(),
        Some(E::Unsupported)
    );
}

#[test]
fn expression_and_identifier_boundaries_do_not_allow_fragment_injection_or_loss() {
    for expression in [
        "1, 2",
        "1 FROM Hidden",
        "1; SELECT 2",
        "1;",
        "1) FROM Hidden --",
        "1) + (2",
        "True) OR False OR (True",
        "[src].*",
    ] {
        assert!(
            matches!(compile(&[column(expression)]), Err(E::Parse(_))),
            "{expression}"
        );
    }
    assert_eq!(
        compile(&[column("1 -- stored comment")]).unwrap().query(),
        parse_query("SELECT 1", Default::default()).unwrap().query()
    );
    let mut field = column("[ID]");
    field.name1 = Some("alias; SELECT x -- 'quote'");
    assert!(compile(&[field]).is_ok());
    let quoted_alias = Fragment {
        name1: Some("[a]"),
        ..column("1")
    };
    assert_eq!(
        compile(&[quoted_alias]).unwrap().query(),
        parse_query("SELECT 1 AS a", Default::default())
            .unwrap()
            .query()
    );
    for name in ["", "a.b", "[[a]]", "a] FROM Hidden --", "a\nb", "a\0b"] {
        let mut field = column("1");
        field.name1 = Some(name);
        assert_eq!(compile(&[field]).err(), Some(E::Unsupported));
    }
    let mut wildcard = column("*");
    wildcard.name1 = Some("alias");
    assert_eq!(compile(&[wildcard]).err(), Some(E::InvalidMetadata));
}

#[test]
fn reconstruction_checks_cumulative_budgets_parser_limits_and_bounded_cancellation() {
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut limited = match resource {
            Resource::Bytes => ResourceBudget::new(0, 100, 100_000),
            Resource::Rows => ResourceBudget::new(10_000_000, 0, 100_000),
            Resource::Steps => ResourceBudget::new(10_000_000, 100, 0),
        };
        assert!(
            matches!(reconstruct_saved_select(definition(&[column("1")], 0), Default::default(), &mut limited, &|| false), Err(E::Limit(error)) if error.resource == resource)
        );
    }
    for limit in [
        ParseLimits {
            max_bytes: 2,
            ..Default::default()
        },
        ParseLimits {
            max_tokens: 1,
            ..Default::default()
        },
        ParseLimits {
            max_depth: 0,
            ..Default::default()
        },
    ] {
        assert!(
            reconstruct_saved_select(
                definition(&[column("1 + 2")], 0),
                limit,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    let mut limited = budget();
    let before = limited.remaining(Resource::Bytes);
    assert_eq!(
        reconstruct_saved_select(
            definition(&[column("1")], 0),
            Default::default(),
            &mut limited,
            &|| true
        )
        .err(),
        Some(E::Cancelled)
    );
    assert_eq!(limited.remaining(Resource::Bytes), before);
    let checks = Cell::new(0);
    reconstruct_saved_select(
        definition(&[column("1")], 0),
        Default::default(),
        &mut budget(),
        &|| {
            checks.set(checks.get() + 1);
            false
        },
    )
    .unwrap();
    let total_checks = checks.get();
    checks.set(0);
    let cancel = || {
        checks.set(checks.get() + 1);
        checks.get() == total_checks
    };
    assert_eq!(
        reconstruct_saved_select(
            definition(&[column("1")], 0),
            Default::default(),
            &mut budget(),
            &cancel
        )
        .err(),
        Some(E::Cancelled)
    );
    let mut second = column("12345678");
    second.order = Some(&[0, 0, 0, 2]);
    assert_eq!(
        reconstruct_saved_select(
            definition(&[column("12345678"), second], 0),
            ParseLimits {
                max_bytes: 16,
                ..Default::default()
            },
            &mut budget(),
            &|| false,
        )
        .err(),
        Some(E::SyntaxLimit)
    );
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut full = budget();
        let before = full.remaining(resource);
        reconstruct_saved_select(
            definition(&[column("1")], 0),
            Default::default(),
            &mut full,
            &|| false,
        )
        .unwrap();
        let consumed = before - full.remaining(resource);
        let mut limited = ResourceBudget::new(
            if resource == Resource::Bytes {
                consumed - 1
            } else {
                64 * 1024 * 1024
            },
            if resource == Resource::Rows {
                consumed - 1
            } else {
                16_384
            },
            if resource == Resource::Steps {
                consumed - 1
            } else {
                1_000_000
            },
        );
        assert!(
            matches!(reconstruct_saved_select(definition(&[column("1")], 0), Default::default(), &mut limited, &|| false), Err(E::Limit(error)) if error.resource == resource)
        );
    }
}

#[test]
fn existing_ace12_saved_source_name_becomes_syntax_without_automatic_source_expansion() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    let name = "~sq_c100_Main_Menu_frm~sq_ccboCREWID";
    let definition = read_saved_query_definition(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        name,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let result =
        reconstruct_saved_select(definition, Default::default(), &mut budget(), &|| false).unwrap();
    assert_eq!(
        result.query(),
        parse_query(
            "SELECT DISTINCTROW * FROM [101f_cbo_Crew_Id_qry]",
            Default::default()
        )
        .unwrap()
        .query()
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn quoted_unicode_aliases_and_literal_expressions_match_an_independent_owned_parse(
        value in any::<i32>(), alias in "[a-zA-Zé🦀' ;=-]{0,24}", distinct in any::<bool>(),
    ) {
        let alias = format!("alias:{alias}");
        let expression = value.to_string();
        let mut flags = row(3); flags.flag = Some(if distinct {2} else {0});
        let mut field = column(&expression); field.name1 = Some(&alias);
        let result = compile(&[flags, field]).unwrap();
        let expected = parse_query(&format!("SELECT {}{expression} AS [{alias}]", if distinct {"DISTINCT "} else {""}), Default::default()).unwrap();
        prop_assert_eq!(result.query(), expected.query());
        prop_assert_eq!(result.definition().rows()[1].name1(), Some(alias.as_str()));
    }
}

#[path = "saved_select/unicode_sources.rs"]
mod unicode_sources;
