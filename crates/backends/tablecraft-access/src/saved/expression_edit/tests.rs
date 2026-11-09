use super::*;
use tablecraft_ace::{QueryAttribute, read_saved_query_definition};
use tablecraft_model::CatalogScope;
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::{
    query_expression_edits as raw, query_sql_expression_edits as sql_raw,
};

const SCOPE: CatalogScope = CatalogScope::new(173);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn reviewed(
    before: &[u8],
    original: &str,
    proposed: &str,
) -> Result<RecoverableNativeQueryExpressionEditsPlan, SavedQuerySqlEditError> {
    let limits = ParseLimits::default();
    let original = parse_query(original, limits).unwrap();
    let proposed = parse_query(proposed, limits).unwrap();
    prepare_saved_query_sql_expression_edits(
        image(before),
        "Saved",
        &original,
        &proposed,
        limits,
        &mut budget(),
        &mut budget(),
        &|| false,
    )
}

#[test]
fn two_projection_sql_edit_matches_complete_raw_after_in_native_projection_order() {
    let before = raw::two_projections();
    let original = "SELECT 42 AS Answer, 99 AS [Second];";
    let proposed = "SELECT 1000 AS Answer, 7000 AS [Second];";
    let plan = reviewed(&before, original, proposed).unwrap();
    let oracle = raw::replace_at(&raw::replace_at(&before, 3, "1000"), 4, "7000");
    assert_eq!(plan.before_bytes(), before);
    assert_eq!(plan.after_bytes(), oracle);
    assert_eq!(plan.intent().edits().len(), 2);
    assert_eq!(plan.intent().edits()[0].as_request().order, [0, 0, 0, 1]);
    assert_eq!(plan.intent().edits()[1].as_request().order, [0, 0, 0, 2]);
}

#[test]
fn projection_and_where_preserve_sources_aliases_and_full_proposed_model() {
    let before = sql_raw::local_where();
    let original = "SELECT 42 AS Answer, 99 AS [Second] FROM MSysObjects AS p WHERE [p].[Id] >= 2;";
    let proposed =
        "SELECT [p].[Id] AS Answer, 99 AS [Second] FROM MSysObjects AS p WHERE [p].[Id] >= 1000;";
    let plan = reviewed(&before, original, proposed).unwrap();
    let oracle = raw::replace_at(
        &raw::replace_at(&before, 4, "[p].[Id]"),
        6,
        "([p].[Id] >= 1000)",
    );
    assert_eq!(plan.after_bytes(), oracle);
    assert_eq!(plan.intent().edits().len(), 2);
    assert_eq!(
        plan.intent().edits()[1].as_request().attribute,
        QueryAttribute::Where
    );

    let actual = read_saved_query_definition(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Saved",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let actual =
        reconstruct_saved_read(actual, ParseLimits::default(), &mut budget(), &|| false).unwrap();
    assert_eq!(
        actual.query(),
        parse_query(proposed, ParseLimits::default())
            .unwrap()
            .query()
    );
}

#[test]
fn unchanged_native_spelling_and_format_only_sql_are_never_rewritten() {
    let before = sql_raw::retained_spelling();
    let original = "SELECT 42 AS Answer, 99 AS [Second];";
    let proposed = "SELECT 42 AS Answer, 7000 AS [Second];";
    let plan = reviewed(&before, original, proposed).unwrap();
    assert_eq!(plan.after_bytes(), raw::replace_at(&before, 4, "7000"));
    assert_eq!(plan.intent().edits().len(), 1);
    let definition = read_saved_query_definition(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Saved",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        definition
            .rows()
            .iter()
            .find(|row| row.attribute() == Some(QueryAttribute::Column)
                && row.order() == Some([0, 0, 0, 1].as_slice()))
            .unwrap()
            .expression(),
        Some("00042")
    );

    let formatted = reviewed(
        &before,
        original,
        " select 42 as [Answer], 99 as [Second] ; ",
    )
    .unwrap();
    assert!(!formatted.changed());
    assert_eq!(formatted.after_bytes(), before);
    assert!(formatted.intent().edits().is_empty());
}

#[test]
fn added_removed_alias_parameter_source_and_where_topology_changes_refuse() {
    let before = sql_raw::local_where();
    let original = "SELECT 42 AS Answer, 99 AS [Second] FROM MSysObjects AS p WHERE [p].[Id] >= 2;";
    for proposed in [
        "SELECT 42 AS Answer FROM MSysObjects AS p WHERE [p].[Id] >= 2;",
        "SELECT 42 AS Answer, 99 AS [Second], 7 AS Third FROM MSysObjects AS p WHERE [p].[Id] >= 2;",
        "SELECT 1000 AS Changed, 99 AS [Second] FROM MSysObjects AS p WHERE [p].[Id] >= 2;",
        "SELECT 42 AS Answer, 99 AS [Second] FROM MSysQueries AS p WHERE [p].[Id] >= 2;",
        "SELECT 42 AS Answer, 99 AS [Second] FROM MSysObjects AS q WHERE [q].[Id] >= 2;",
        "SELECT 42 AS Answer, 99 AS [Second] FROM MSysObjects AS p;",
        "PARAMETERS [n] LONG; SELECT 42 AS Answer, 99 AS [Second] FROM MSysObjects AS p WHERE [p].[Id] >= [n];",
        "SELECT DISTINCT 42 AS Answer, 99 AS [Second] FROM MSysObjects AS p WHERE [p].[Id] >= 2;",
    ] {
        assert!(matches!(
            reviewed(&before, original, proposed),
            Err(SavedQuerySqlEditError::Unsupported)
        ));
    }
    assert!(matches!(
        reviewed(
            &before,
            "SELECT 43 AS Answer, 99 AS [Second] FROM MSysObjects AS p WHERE [p].[Id] >= 2;",
            original
        ),
        Err(SavedQuerySqlEditError::OriginalChanged)
    ));
}

#[test]
fn oversized_or_nested_source_expression_changes_never_publish_partial_plan() {
    let before = raw::two_projections();
    let original = "SELECT 42 AS Answer, 99 AS [Second];";
    assert!(
        reviewed(
            &before,
            original,
            "SELECT 1000 AS Answer, 'abcdefghijklmnopqrstuvwxyz0123456789' AS [Second];"
        )
        .is_err()
    );
    assert!(matches!(
        reviewed(
            &before,
            original,
            "SELECT (SELECT 1 FROM [OpaqueSource]) AS Answer, 99 AS [Second];"
        ),
        Err(SavedQuerySqlEditError::Unsupported)
    ));
    assert_eq!(before, raw::two_projections());
}

#[test]
fn independent_pools_cancellation_and_parser_output_limits_are_enforced() {
    let before = raw::two_projections();
    let original = parse_query(
        "SELECT 42 AS Answer, 99 AS [Second];",
        ParseLimits::default(),
    )
    .unwrap();
    let proposed = parse_query(
        "SELECT 1000 AS Answer, 99 AS [Second];",
        ParseLimits::default(),
    )
    .unwrap();
    for native_empty in [false, true] {
        let mut native = if native_empty {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let mut syntax = if native_empty {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        assert!(
            prepare_saved_query_sql_expression_edits(
                image(&before),
                "Saved",
                &original,
                &proposed,
                ParseLimits::default(),
                &mut native,
                &mut syntax,
                &|| false
            )
            .is_err()
        );
    }
    assert!(
        prepare_saved_query_sql_expression_edits(
            image(&before),
            "Saved",
            &original,
            &proposed,
            ParseLimits::default(),
            &mut budget(),
            &mut budget(),
            &|| true
        )
        .is_err()
    );
    assert!(
        prepare_saved_query_sql_expression_edits(
            image(&before),
            "Saved",
            &original,
            &proposed,
            ParseLimits {
                max_bytes: 8,
                ..ParseLimits::default()
            },
            &mut budget(),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn changed_quoted_text_uses_owned_sql_roundtrip_without_native_text_slicing() {
    let before = raw::two_projections();
    let plan = reviewed(
        &before,
        "SELECT 42 AS Answer, 99 AS [Second];",
        "SELECT 'a''b' AS Answer, 99 AS [Second];",
    )
    .unwrap();
    assert_eq!(plan.after_bytes(), raw::replace_at(&before, 3, "'a''b'"));
    assert_eq!(plan.intent().edits().len(), 1);
    assert_eq!(plan.intent().edits()[0].as_request().replacement, "'a''b'");
}

#[test]
fn unrepresentable_original_alias_is_refused_instead_of_quoted_or_repaired() {
    let before = sql_raw::source_with_unrepresentable_alias();
    assert!(matches!(
        reviewed(
            &before,
            "SELECT 42 AS Answer, 99 AS [Second];",
            "SELECT 1000 AS Answer, 99 AS [Second];",
        ),
        Err(SavedQuerySqlEditError::Metadata(
            SavedReadError::Unsupported
        ))
    ));
}
