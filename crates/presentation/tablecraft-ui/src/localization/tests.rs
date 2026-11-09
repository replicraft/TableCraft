mod borrowed_arguments;

use super::*;
use fluent_syntax::{ast, parser};
use std::collections::{BTreeMap, BTreeSet};

type Variables = BTreeSet<&'static str>;

fn messages(locale: Locale) -> BTreeMap<&'static str, Variables> {
    let mut messages = BTreeMap::new();
    for (section, source) in catalogs::sources(locale) {
        let resource = parser::parse(source)
            .unwrap_or_else(|(_, errors)| panic!("{locale}/{section}: {errors:?}"));
        let mut section_count = 0;
        for entry in resource.body {
            let ast::Entry::Message(message) = entry else {
                continue;
            };
            let value = message
                .value
                .expect("every UI key must have an actual translated value");
            let mut variables = Variables::new();
            pattern_variables(&value, &mut variables);
            for attribute in message.attributes {
                pattern_variables(&attribute.value, &mut variables);
            }
            assert!(
                messages.insert(message.id.name, variables).is_none(),
                "duplicate key in {locale}/{section}"
            );
            section_count += 1;
        }
        assert!(section_count > 0, "empty {locale}/{section}");
    }
    messages
}

fn pattern_variables(pattern: &ast::Pattern<&'static str>, variables: &mut Variables) {
    for element in &pattern.elements {
        if let ast::PatternElement::Placeable { expression } = element {
            expression_variables(expression, variables);
        }
    }
}

fn expression_variables(expression: &ast::Expression<&'static str>, variables: &mut Variables) {
    match expression {
        ast::Expression::Inline(inline) => inline_variables(inline, variables),
        ast::Expression::Select { selector, variants } => {
            inline_variables(selector, variables);
            for variant in variants {
                pattern_variables(&variant.value, variables);
            }
        }
    }
}

fn inline_variables(expression: &ast::InlineExpression<&'static str>, variables: &mut Variables) {
    match expression {
        ast::InlineExpression::VariableReference { id } => {
            variables.insert(id.name);
        }
        ast::InlineExpression::FunctionReference { arguments, .. } => {
            argument_variables(arguments, variables);
        }
        ast::InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => argument_variables(arguments, variables),
        ast::InlineExpression::Placeable { expression } => {
            expression_variables(expression, variables);
        }
        _ => {}
    }
}

fn argument_variables(arguments: &ast::CallArguments<&'static str>, variables: &mut Variables) {
    for argument in &arguments.positional {
        inline_variables(argument, variables);
    }
    for argument in &arguments.named {
        inline_variables(&argument.value, variables);
    }
}

#[test]
fn every_section_has_complete_keys_variables_and_successful_translations() {
    let english = messages(Locale::EnUs);
    assert!(english.len() >= 307);
    for locale in Locale::ALL {
        let translated = messages(locale);
        assert_eq!(translated, english, "key/variable coverage for {locale}");
        let localizer = Localizer::new(locale).unwrap();
        for (id, variables) in &translated {
            let mut arguments = FluentArgs::new();
            for name in variables {
                if matches!(
                    *name,
                    "count"
                        | "first"
                        | "last"
                        | "number"
                        | "row"
                        | "parent"
                        | "precision"
                        | "scale"
                        | "missing"
                        | "formatting"
                        | "fallbacks"
                        | "units"
                ) {
                    arguments.set(*name, 2);
                } else {
                    arguments.set(*name, "literal value");
                }
            }
            assert!(
                !localizer.format(id, &arguments).is_empty(),
                "{locale}/{id}"
            );
        }
        assert_eq!(localizer.diagnostics(), LocalizationDiagnostics::default());
    }
}

#[test]
fn explicit_locale_selection_uses_native_names_and_japanese_alias() {
    assert_eq!("jp".parse::<Locale>().unwrap(), Locale::Ja);
    assert_eq!(Locale::Ja.as_str(), "ja");
    assert!("unknown".parse::<Locale>().is_err());
    for locale in Locale::ALL {
        assert_eq!(locale.as_str().parse::<Locale>().unwrap(), locale);
        assert!(!locale.native_name().is_empty());
    }
    assert_eq!(Localizer::default().text("shell-ready"), "Ready");
    assert_eq!(
        Localizer::new(Locale::De).unwrap().text("shell-ready"),
        "Bereit"
    );
    assert_eq!(
        Localizer::new(Locale::Es).unwrap().text("ribbon-file"),
        "Archivo"
    );
}

#[test]
fn exceptional_english_fallback_and_format_failures_are_diagnosed_without_payloads() {
    let localizer = Localizer(Arc::new(Shared {
        locale: Locale::Es,
        bundle: Bundle::new_concurrent(vec!["es".parse().unwrap()]),
        english: Some(build_bundle(Locale::EnUs).unwrap()),
        diagnostics: Counters::default(),
    }));
    assert_eq!(localizer.text("shell-ready"), "Ready");
    assert_eq!(localizer.diagnostics().missing_messages, 1);
    assert_eq!(localizer.diagnostics().english_fallbacks, 1);

    let localizer = Localizer::new(Locale::De).unwrap();
    assert_eq!(
        localizer.text("not-a-catalog-key"),
        "Übersetzung nicht verfügbar"
    );
    assert!(localizer.diagnostics().missing_messages > 0);
    assert_eq!(localizer.diagnostics().english_fallbacks, 0);

    assert_eq!(
        localizer.text("datasheet-column-count"),
        "Übersetzung nicht verfügbar"
    );
    assert!(localizer.diagnostics().formatting_errors > 0);
}

#[test]
fn cloned_localizers_support_concurrent_formatting_without_changing_literal_arguments() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Localizer>();

    let localizer = Localizer::new(Locale::Ja).unwrap();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let localizer = localizer.clone();
            scope.spawn(move || {
                let mut arguments = FluentArgs::new();
                arguments.set("name", "{ $other } · SELECT * FROM [データ]");
                let text = localizer.format("datasheet-field-name", &arguments);
                assert!(text.contains("{ $other } · SELECT * FROM [データ]"));
            });
        }
    });
    assert_eq!(localizer.diagnostics(), LocalizationDiagnostics::default());
}
