//! Actual owned SQL models retain independently authored native object bytes.
use super::*;
use tablecraft_ace::QueryCreationError;
use tablecraft_model::CatalogScope;
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::saved_queries::creation as raw;

const SCOPE: CatalogScope = CatalogScope::new(193);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn at() -> LegacyDateTime {
    LegacyDateTime::from_ole_days(45000.5).unwrap()
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn options(name: &str) -> SavedQueryCreationOptions<'_> {
    SavedQueryCreationOptions {
        destination: name,
        captured_at: at(),
        limits: ParseLimits::default(),
    }
}

#[test]
fn owned_signed_constants_and_aliases_match_complete_independent_creation_images() {
    for (sql, name, alias, value) in [
        ("SELECT 43 AS Answer", "New query", "Answer", 43),
        (
            "SELECT -7 AS [Total value]",
            "Résumé 新🦀",
            "Total value",
            -7,
        ),
        (
            "SELECT -2147483648 AS Minimum",
            "Minimum query",
            "Minimum",
            i32::MIN,
        ),
        (
            "SELECT 2147483647 AS Maximum",
            "Maximum query",
            "Maximum",
            i32::MAX,
        ),
        (
            "SELECT -7 AS [\u{feff}Heading]",
            "\u{feff}Created",
            "\u{feff}Heading",
            -7,
        ),
    ] {
        let raw = raw::source(name, alias, value, at());
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare_saved_query_creation(
            image(&raw.before),
            parsed.query(),
            options(name),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.before_bytes(), raw.before);
        assert_eq!(plan.after_bytes(), raw.expected);
        assert_eq!(plan.native_query_id(), raw.native_query_id);
        assert_eq!(plan.intent().value(), value);
        assert_eq!(plan.intent().alias(), alias);
        assert_eq!(plan.intent().captured_at(), at());

        let original = read_saved_query_definition(
            &mut Cursor::new(plan.after_bytes()),
            SCOPE,
            "Saved",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(original.rows()[3].expression(), Some("42"));
        let actual = read_saved_query_definition(
            &mut Cursor::new(plan.after_bytes()),
            SCOPE,
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let actual =
            reconstruct_saved_read(actual, ParseLimits::default(), &mut budget(), &|| false)
                .unwrap();
        assert_eq!(actual.query(), parsed.query());
    }
}

#[test]
fn save_as_consumes_actual_reconstructed_stored_model_without_fabricating_parsed_sql() {
    let raw = raw::source("Copy", "Answer", 42, at());
    let source = image(&raw.before);
    let original =
        read_saved_query_definition(&mut source.reader(), SCOPE, "Saved", &mut budget(), &|| {
            false
        })
        .unwrap();
    let original =
        reconstruct_saved_read(original, ParseLimits::default(), &mut budget(), &|| false).unwrap();
    let plan = prepare_saved_query_creation(
        source,
        original.query(),
        options("Copy"),
        &mut budget(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(plan.before_bytes(), raw.before);
    assert_eq!(plan.after_bytes(), raw.expected);
    let saved = read_saved_query_definition(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Saved",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        saved.origin().native_id(),
        original.definition().origin().native_id()
    );
    let saved =
        reconstruct_saved_read(saved, ParseLimits::default(), &mut budget(), &|| false).unwrap();
    assert_eq!(saved.query(), original.query());
    assert_eq!(plan.intent().name(), "Copy");
    assert_ne!(
        plan.native_query_id(),
        original.definition().origin().native_id()
    );
}

#[test]
fn unsupported_owned_forms_refuse_without_evaluating_expressions_or_constructing_native_plan() {
    let raw = raw::source("Unused", "Answer", 43, at());
    for sql in [
        "SELECT 43",
        "SELECT 2147483648 AS Answer",
        "SELECT 1 / 0 AS Answer",
        "SELECT CLng(1 / 0) AS Answer",
        "SELECT [Missing] AS Answer",
        "SELECT 43 AS Answer FROM Saved",
        "SELECT 43 AS Answer WHERE FALSE",
        "SELECT DISTINCT 43 AS Answer",
        "SELECT TOP 1 43 AS Answer",
        "SELECT 43 AS Answer ORDER BY 1",
        "SELECT 43 AS Answer UNION ALL SELECT 43 AS Answer",
        "PARAMETERS [Input] LONG; SELECT 43 AS Answer",
        "SELECT 43 AS Answer INTO Other FROM MSysObjects",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        // Zero native Bytes would reject any actual native declaration capture.
        let result = prepare_saved_query_creation(
            image(&raw.before),
            parsed.query(),
            options("Unused"),
            &mut ResourceBudget::new(0, 1_000_000, 500_000_000),
            &mut budget(),
            &|| false,
        );
        assert!(
            matches!(result, Err(SavedQueryCreationError::Unsupported)),
            "{sql}: unsupported shape must not evaluate or reach native construction"
        );
    }
}

#[test]
fn native_and_syntax_failures_keep_their_actual_pools_and_typed_causes() {
    let raw = raw::source("Created", "Answer", 43, at());
    let parsed = parse_query("SELECT 43 AS Answer", ParseLimits::default()).unwrap();
    let native_failure = prepare_saved_query_creation(
        image(&raw.before),
        parsed.query(),
        options("Created"),
        &mut ResourceBudget::new(0, 1_000_000, 500_000_000),
        &mut budget(),
        &|| false,
    );
    assert!(matches!(
        native_failure,
        Err(SavedQueryCreationError::Intent(
            QueryCreationIntentError::Native(QueryCreationError::Inspect(InspectError::Limit(_)))
        ))
    ));
    let syntax_failure = prepare_saved_query_creation(
        image(&raw.before),
        parsed.query(),
        options("Created"),
        &mut budget(),
        &mut ResourceBudget::new(0, 1_000_000, 500_000_000),
        &|| false,
    );
    assert!(matches!(
        syntax_failure,
        Err(SavedQueryCreationError::Metadata(SavedReadError::Limit(_)))
    ));
    let cancelled = prepare_saved_query_creation(
        image(&raw.before),
        parsed.query(),
        options("Created"),
        &mut budget(),
        &mut budget(),
        &|| true,
    );
    assert!(matches!(
        cancelled,
        Err(SavedQueryCreationError::Metadata(SavedReadError::Cancelled))
    ));
}
