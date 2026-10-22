use super::*;

#[test]
fn bundled_catalog_uses_classic_titles_and_a_distinct_undefined_fallback() {
    fn require_send_sync<T: Send + Sync>() {}
    require_send_sync::<FluentErrorMessages>();
    require_send_sync::<EnglishErrorMessages>();
    let messages = EnglishErrorMessages::default();
    for (code, expected) in [
        (5, "Invalid procedure call or argument"),
        (6, "Overflow"),
        (11, "Division by zero"),
        (13, "Type mismatch"),
        (47, "Too many DLL application clients"),
        (1, "Application-defined or object-defined error"),
        (65535, "Application-defined or object-defined error"),
    ] {
        assert_eq!(messages.message(code), Ok(expected));
        assert_eq!(messages.message(code), Ok(expected));
    }
    let count = include_str!("../../../data/error_messages/en-US.ftl")
        .lines()
        .filter(|line| line.starts_with("error-") && !line.starts_with("error-unknown"))
        .count();
    assert_eq!(count, 86);
}

#[test]
fn resources_are_explicit_flat_bounded_and_do_not_fall_back_to_another_language() {
    let french = FluentErrorMessages::new(
        "fr",
        "error-unknown = Inconnue\nerror-11 = Division par zéro\n",
    )
    .unwrap();
    assert_eq!(french.message(11), Ok("Division par zéro"));
    assert_eq!(french.message(13), Ok("Inconnue"));
    for resource in [
        "error-11 = No fallback",
        "error-unknown = { $argument }",
        "error-unknown = { error-11 }\nerror-11 = Referenced",
        "error-unknown = { NUMBER(11) }",
        "error-unknown = Text\n    .attribute = Nope",
        "error-unknown = Text\n-term = Nope",
        "error-unknown = Text\nerror-0 = Nope",
        "error-unknown = Text\nerror-01 = Nope",
        "error-unknown = Text\nerror-65536 = Nope",
        "error-unknown = Text\nother = Nope",
        "error-unknown = Text\nerror-11 = One\nerror-11 = Two",
        "error-unknown = One\nerror-unknown = Two",
        "error-unknown = {",
    ] {
        assert!(
            matches!(
                FluentErrorMessages::new("en-US", resource),
                Err(EvalError::InvalidErrorCatalog)
            ),
            "{resource}"
        );
    }
    assert!(matches!(
        FluentErrorMessages::new("not_a_locale", "error-unknown = Text"),
        Err(EvalError::InvalidErrorCatalog)
    ));
    let nested = format!(
        "error-unknown = {}x{}",
        "{".repeat(20_000),
        "}".repeat(20_000)
    );
    assert!(matches!(
        FluentErrorMessages::new("en-US", &nested),
        Err(EvalError::InvalidErrorCatalog)
    ));
    let oversized = "x".repeat(131_073);
    assert!(matches!(
        FluentErrorMessages::new("en-US", &oversized),
        Err(EvalError::InvalidErrorCatalog)
    ));
}
