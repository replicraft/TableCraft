use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as base;

use std::{cell::Cell, io::Cursor};
use support::data_page;
use tablecraft_ace::{
    ConstraintReport, ConstraintViolation, InspectError, RuleCheckStatus, check_table_constraints,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::EvalError;
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const TABLE_PAGE: usize = 10;
const DATA_PAGE: usize = 12;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}

fn database(rule: &str, values: &[Option<&str>]) -> Vec<u8> {
    let mut bytes = base::with_rule("ValidationRule", rule);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    let definition = base::definition(
        &[("Id", 4, 0, 4), ("Score", 10, 0, 510)],
        values.len() as u32,
        11 << 8,
    );
    bytes[TABLE_PAGE * PAGE_BYTES..(TABLE_PAGE + 1) * PAGE_BYTES].copy_from_slice(&definition);
    let rows = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend((index as i32).to_le_bytes());
            if let Some(value) = value {
                row.extend(value.encode_utf16().flat_map(u16::to_le_bytes));
            }
            row.extend((row.len() as u16).to_le_bytes());
            row.extend(6_u16.to_le_bytes());
            row.extend(1_u16.to_le_bytes());
            row.push(if value.is_some() { 3 } else { 1 });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(TABLE_PAGE as u32, &rows));
    bytes
}

fn audit(bytes: &[u8]) -> Result<ConstraintReport, InspectError> {
    check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
}

#[test]
fn len_checks_utf16_units_and_pure_text_concatenation_without_formatting() {
    for rule in [
        "Len([Score]) = 2",
        "lEn([Items].[Score] & '') Between 2 And 2",
        "Len([Score] + '') In (2, 3)",
    ] {
        let bytes = database(rule, &[Some("é中"), Some("😀"), Some("\0A")]);
        assert_eq!(
            audit(&bytes).unwrap().fields()[1].validation(),
            RuleCheckStatus::Checked,
            "{rule}"
        );
    }
    assert_eq!(
        audit(&database("Len([Score]) < 2", &[Some("😀")])).err(),
        Some(InspectError::Constraint(ConstraintViolation::FieldRule))
    );
}

#[test]
fn binary_strcomp_is_case_sensitive_and_preserves_trailing_spaces() {
    for (value, expected) in [("ABC", true), ("abc", false), ("ABC ", false)] {
        let result = audit(&database("sTrCoMp([Score], 'ABC', 0) = 0", &[Some(value)]));
        if expected {
            assert_eq!(
                result.unwrap().fields()[1].validation(),
                RuleCheckStatus::Checked
            );
        } else {
            assert_eq!(
                result.err(),
                Some(InspectError::Constraint(ConstraintViolation::FieldRule))
            );
        }
    }
    assert_eq!(
        audit(&database("StrComp([Score], 'a', 0) < 0", &[Some("A")]))
            .unwrap()
            .fields()[1]
            .validation(),
        RuleCheckStatus::Checked
    );
}

#[test]
fn null_results_stay_unsupported_and_explicit_guards_use_eager_evaluation() {
    for rule in ["Len([Score]) < 100", "StrComp([Score], 'ABC', 0) = 0"] {
        assert_eq!(
            audit(&database(rule, &[None])).unwrap().fields()[1].validation(),
            RuleCheckStatus::Unsupported
        );
    }
    for rule in [
        "[Score] Is Null Or Len([Score]) < 100",
        "Len([Score] & '') = 0",
        "StrComp([Score] & '', '', 0) = 0",
    ] {
        assert_eq!(
            audit(&database(rule, &[None])).unwrap().fields()[1].validation(),
            RuleCheckStatus::Checked
        );
    }
    assert_eq!(
        audit(&database("Len([Score] + '') = 0", &[None]))
            .unwrap()
            .fields()[1]
            .validation(),
        RuleCheckStatus::Unsupported
    );
    assert_eq!(
        audit(&database("Len([Score]) < 0 And 1 / 0 > 0", &[Some("A")])).err(),
        Some(InspectError::Constraint(ConstraintViolation::Evaluation(
            EvalError::DivisionByZero
        )))
    );
}

#[test]
fn unsupported_signatures_scopes_and_conversions_are_preflighted_even_when_empty() {
    for rule in [
        "Len([Id]) > 0",
        "Len([Score] & [Id]) > 0",
        "Len([Score] + 1) > 0",
        "Len(#2020-01-01#) > 0",
        "Len([Other].[Score]) > 0",
        "Len([Missing]) > 0",
        "Len() > 0",
        "Len([Score], 1) > 0",
        "VBA.Len([Score]) > 0",
        "StrComp([Score], 'A') = 0",
        "StrComp([Score], 'A', 1) = 0",
        "StrComp([Score], 'A', 2) = 0",
        "StrComp([Score], 'A', NULL) = 0",
        "StrComp([Score], 'A', '0') = 0",
        "StrComp([Score], 'A', 0 + 0) = 0",
        "StrComp([Score], 1, 0) = 0",
        "StrComp(UCase([Score]), [Score], 0) = 0",
        "Len(Dangerous()) > 0",
        "Len((SELECT 'A')) > 0",
        "Len([Score]) = Len([Id])",
    ] {
        for values in [&[][..], &[Some("A")][..]] {
            assert_eq!(
                audit(&database(rule, values)).unwrap().fields()[1].validation(),
                RuleCheckStatus::Unsupported,
                "{rule}"
            );
        }
    }
    assert_eq!(
        audit(&database("Len([Score]) < 100", &[]))
            .unwrap()
            .fields()[1]
            .validation(),
        RuleCheckStatus::Checked
    );
}

#[test]
fn function_audits_remain_bounded_cancelled_and_read_only() {
    let original = database("Len([Score]) < 100", &[Some("A"), Some("😀")]);
    let bytes = original.clone();
    let calls = Cell::new(0);
    let count = || {
        calls.set(calls.get() + 1);
        false
    };
    check_table_constraints(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &count,
    )
    .unwrap();
    let stop = calls.get() - 5;
    calls.set(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() >= stop
    };
    assert_eq!(
        check_table_constraints(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &cancel,
        )
        .err(),
        Some(InspectError::Cancelled)
    );
    for mut limited in [
        ResourceBudget::new(0, 16_384, 1_000_000),
        ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(16 * 1024 * 1024, 16_384, 0),
    ] {
        assert!(matches!(
            check_table_constraints(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut limited,
                &|| false,
            ),
            Err(InspectError::Limit(_))
        ));
    }
    assert_eq!(bytes, original);
}
