use super::*;
use crate::evaluation::{EvalLimits, Evaluator, NoBindings};
use crate::functions::text::TextFunction;
use proptest::prelude::*;
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::Double};

#[test]
fn documented_components_display_fallback_and_full_addresses_are_preserved() {
    for (text, expected) in [
        (
            "Microsoft#https://www.microsoft.com#",
            [
                "Microsoft",
                "Microsoft",
                "https://www.microsoft.com",
                "",
                "",
                "https://www.microsoft.com",
            ],
        ),
        (
            "#https://www.microsoft.com#",
            [
                "https://www.microsoft.com",
                "",
                "https://www.microsoft.com",
                "",
                "",
                "https://www.microsoft.com",
            ],
        ),
        (
            "Customers#https://www.microsoft.com#Form Customers",
            [
                "Customers",
                "Customers",
                "https://www.microsoft.com",
                "Form Customers",
                "",
                "https://www.microsoft.com#Form Customers",
            ],
        ),
        (
            "##Form Customers#Enter Information",
            [
                "Form Customers",
                "",
                "",
                "Form Customers",
                "Enter Information",
                "#Form Customers",
            ],
        ),
        ("label", ["label", "label", "", "", "", ""]),
        ("", ["", "", "", "", "", ""]),
        (
            "名🦀#résumé#中#tip#tail",
            ["名🦀", "名🦀", "résumé", "中", "tip#tail", "résumé#中"],
        ),
    ] {
        for (selector, expected) in expected.into_iter().enumerate() {
            let part = HyperlinkPart::try_from(selector as i32).unwrap();
            assert_eq!(extract(text, part, || Ok(())), Ok(expected));
        }
    }
}

#[test]
fn query_arity_selector_conversion_and_null_policy_are_explicit() {
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::HyperlinkPart,
            &[
                Value::Text("name#address#sub#tip".into()),
                Value::Double(Double::from_float(2.5)),
            ]
        ),
        Ok(Value::Text("address".into()))
    );
    assert_eq!(
        evaluator.apply_text_function(TextFunction::HyperlinkPart, &[Value::Null, Value::Byte(2)]),
        Ok(Value::Null)
    );
    for (selector, expected) in [
        (Value::Int32(-1), EvalError::Domain),
        (Value::Int32(6), EvalError::Domain),
        (Value::Null, EvalError::InvalidNullConversion),
        (Value::Text("2".into()), EvalError::TypeMismatch),
    ] {
        assert_eq!(
            evaluator.apply_text_function(TextFunction::HyperlinkPart, &[Value::Null, selector]),
            Err(expected)
        );
    }
    assert!(!TextFunction::HyperlinkPart.accepts_arity(1));
    assert!(!TextFunction::HyperlinkPart.accepts_arity(3));
}

#[test]
fn scanning_cancellation_and_output_limits_apply_to_borrowed_components() {
    let text = "long display#address#sub#tip";
    let calls = Cell::new(0);
    extract(text, HyperlinkPart::FullAddress, || {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();

    for stop in 1..=calls.get() {
        let count = Cell::new(0);
        assert_eq!(
            extract(text, HyperlinkPart::FullAddress, || {
                count.set(count.get() + 1);
                if count.get() == stop {
                    Err(EvalError::Cancelled)
                } else {
                    Ok(())
                }
            }),
            Err(EvalError::Cancelled)
        );
    }

    let mut budget = ResourceBudget::new(0, 0, 1000);
    let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .apply_text_function(
            TextFunction::HyperlinkPart,
            &[Value::Text(text.into()), Value::Byte(2)],
        );
    assert!(matches!(result, Err(EvalError::Limit(_))));
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn generated_fields_round_trip_all_selectors(display in "[^#]{0,20}", address in "[^#]{0,20}", sub in "[^#]{0,20}", tip in ".{0,20}") {
        let text = format!("{display}#{address}#{sub}#{tip}");
        let full = if sub.is_empty() { address.clone() } else { format!("{address}#{sub}") };
        let displayed = [&display, &address, &sub].into_iter().find(|part| !part.is_empty()).cloned().unwrap_or_default();

        for (selector, expected) in [displayed, display, address, sub, tip, full].into_iter().enumerate() {
            let actual = extract(&text, HyperlinkPart::try_from(selector as i32).unwrap(), || Ok(())).unwrap();
            prop_assert_eq!(actual, expected);
        }
    }
}
