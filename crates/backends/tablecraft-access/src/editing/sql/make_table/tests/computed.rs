//! Known fixed expression carriers become manual native fields, not inferred rows.
use super::budget;
use super::{image, prepare, syntax};
use crate::editing::sql::NativeMakeTableError;
use crate::editing::sql::prepare_native_make_table;
use std::cell::RefCell;
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime, Value};
use tablecraft_expressions::{EvalError, functions::dates::clock::ClockReading};
use tablecraft_query::{ExecutionOptions, MakeTableError, QueryError};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{fields, populated};

fn source() -> (Vec<u8>, Vec<Option<i32>>) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let mut input = vec![Some(23)];
    input.extend((0..70).map(Some));
    let values = input
        .iter()
        .map(|value| value.map(|value| value + 23))
        .collect();
    (populated::numbers(&seed, "Items", &input), values)
}

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None
    );
}

#[test]
fn fixed_expression_stages_more_than_sixty_four_rows_with_no_native_policy_inheritance() {
    let (before, values) = source();
    let staged = prepare(
        &before,
        "SELECT CLng(i.CopiedScore+23) AS CopiedScore INTO Archive FROM Items AS i",
    )
    .unwrap();
    let expected: Vec<_> = values
        .iter()
        .map(|value| vec![Value::Int32(value.unwrap())])
        .collect();
    assert_eq!(staged.rows(), expected);
    assert_eq!(staged.definition().column_count(), 1);
    assert!(staged.definition().is_manual());
    assert!(staged.definition().is_native());
    assert_eq!(staged.definition().primary_position(), None);
    assert_eq!(staged.definition().primary_name(), None);
    same(staged.original().as_bytes(), &before);
    #[cfg(feature = "journal")]
    {
        let after = populated::numbers(&before, "Archive", &values);
        let native = staged.finish(&mut budget(), &|| false).unwrap();
        same(native.before_bytes(), &before);
        same(native.after_bytes(), &after);
    }
}

#[test]
fn known_empty_and_nullable_computed_outputs_keep_declared_type_and_null_carriers() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(&seed, "Items", &[Some(23), None]);
    for (sql, expected) in [
        (
            "SELECT CLng(i.CopiedScore+23) AS CopiedScore INTO Archive FROM Items AS i WHERE i.CopiedScore<0",
            vec![],
        ),
        (
            "SELECT IIf(i.CopiedScore IS NULL,Null,CLng(7)) AS CopiedScore INTO Archive FROM Items AS i",
            vec![Some(7), None],
        ),
    ] {
        let staged = prepare(&before, sql).unwrap();
        let rows: Vec<_> = expected
            .iter()
            .map(|value| vec![value.map_or(Value::Null, Value::Int32)])
            .collect();
        assert_eq!(staged.rows(), rows);
        assert_eq!(staged.definition().column_count(), 1);
        assert_eq!(staged.definition().primary_position(), None);
        #[cfg(feature = "journal")]
        {
            let after = populated::numbers(&before, "Archive", &expected);
            let native = staged.finish(&mut budget(), &|| false).unwrap();
            same(native.after_bytes(), &after);
        }
    }
}

#[test]
fn computed_text_dynamic_null_only_unaliased_and_parameter_shapes_are_not_guessed() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(&seed, "Items", &[Some(23)]);
    for sql in [
        "SELECT CStr(i.CopiedScore) AS Label INTO Archive FROM Items AS i",
        "SELECT 'a🦀' AS Label INTO Archive FROM Items AS i",
        "SELECT CDec(i.CopiedScore) AS Amount INTO Archive FROM Items AS i",
        "SELECT CLng(i.CopiedScore) INTO Archive FROM Items AS i",
        "SELECT CLng(DLookup('Score','Items')) AS CopiedScore INTO Archive FROM Items AS i",
        "SELECT CLng((SELECT Max(CopiedScore) FROM Items)) AS CopiedScore INTO Archive FROM Items AS i",
        "PARAMETERS p LONG; SELECT CLng(i.CopiedScore) AS CopiedScore INTO Archive FROM Items AS i WHERE i.CopiedScore=p",
    ] {
        let result = prepare(&before, sql);
        assert!(
            matches!(&result, Err(NativeMakeTableError::Shape)),
            "{sql}: {:?}",
            result.err()
        );
    }
    for sql in [
        "SELECT Null AS CopiedScore INTO Archive FROM Items AS i",
        "SELECT IIf(i.CopiedScore=23,i.CopiedScore,'other') AS CopiedScore INTO Archive FROM Items AS i",
    ] {
        assert!(
            matches!(
                prepare(&before, sql),
                Err(NativeMakeTableError::Query(
                    MakeTableError::IndeterminateType
                ))
            ),
            "{sql}"
        );
    }
    assert!(matches!(
        prepare(
            &before,
            "SELECT 1/0 AS CopiedScore INTO Items FROM Items AS i"
        ),
        Err(NativeMakeTableError::DestinationExists)
    ));
}

#[test]
fn a_late_computed_error_discards_the_complete_relation_instead_of_staging_a_prefix() {
    let (before, _) = source();
    for sql in [
        "SELECT CLng(1/(i.CopiedScore-69)) AS CopiedScore INTO Archive FROM Items AS i",
        "SELECT TOP 0 CLng(1/(i.CopiedScore-69)) AS CopiedScore INTO Archive FROM Items AS i",
    ] {
        let result = prepare(&before, sql);
        assert!(
            matches!(
                &result,
                Err(NativeMakeTableError::Query(MakeTableError::Query(
                    QueryError::Evaluation(EvalError::DivisionByZero)
                )))
            ),
            "{sql}: {:?}",
            result.err()
        );
    }
}

#[test]
fn a_computed_boolean_keeps_mask_values_without_a_nullable_or_default_substitute() {
    let (before, _) = source();
    let staged = prepare(
        &before,
        "SELECT (i.CopiedScore>0) AS Enabled INTO Archive FROM Items AS i",
    )
    .unwrap();
    let mut expected = vec![true];
    expected.extend((0..70).map(|value| value > 0));
    let rows: Vec<_> = expected
        .iter()
        .map(|value| vec![Value::Boolean(*value)])
        .collect();
    assert_eq!(staged.rows(), rows);
    assert_eq!(staged.definition().primary_position(), None);
    #[cfg(feature = "journal")]
    {
        let after = populated::booleans(&before, "Archive", &expected);
        let native = staged.finish(&mut budget(), &|| false).unwrap();
        same(native.after_bytes(), &after);
    }
}

#[test]
fn scalar_registry_math_financial_inspection_and_closed_eval_are_not_arbitrarily_filtered() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(&seed, "Items", &[Some(-23)]);
    for (expression, value) in [
        ("CLng(Abs(i.CopiedScore))", 23),
        ("CLng(Sin(0))", 0),
        ("CLng(Pmt(0,2,10,,0))", -5),
        ("CLng(NPV(0,Array(2,3)))", 5),
        ("CLng(Eval('1+2'))", 3),
        ("CLng(IIf(IsNull(i.CopiedScore),0,1))", 1),
    ] {
        let sql = format!("SELECT {expression} AS CopiedScore INTO Archive FROM Items AS i");
        let staged = prepare(&before, &sql).unwrap();
        assert_eq!(staged.rows(), &[vec![Value::Int32(value)]]);
        #[cfg(feature = "journal")]
        {
            let after = populated::numbers(&before, "Archive", &[Some(value)]);
            let native = staged.finish(&mut budget(), &|| false).unwrap();
            same(native.after_bytes(), &after);
        }
    }
}

#[test]
fn date_add_and_supplied_clock_outputs_keep_exact_manual_datetime_fields() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(&seed, "Items", &[Some(23)]);
    let clock = ClockReading::new(ExtendedDateTime::new(1900, 1, 1, 12, 0, 0, 0).unwrap()).unwrap();
    for (expression, days) in [("DateAdd('d',1,#1/1/1900#)", 3.0), ("Now()", 2.5)] {
        let query = syntax(&format!(
            "SELECT {expression} AS Occurred INTO Archive FROM Items AS i"
        ));
        let staged = prepare_native_make_table(
            image(&before),
            query,
            &RefCell::new(budget()),
            &mut budget(),
            &[],
            ExecutionOptions {
                clock: Some(&clock),
                ..Default::default()
            },
            Default::default(),
        )
        .unwrap();
        assert_eq!(
            staged.rows(),
            &[vec![Value::DateTime(
                LegacyDateTime::from_ole_days(days).unwrap()
            )]]
        );
        assert_eq!(staged.definition().primary_position(), None);
        #[cfg(feature = "journal")]
        {
            let after = populated::computed::dates(&before, "Archive", &[Some(days)]);
            let native = staged.finish(&mut budget(), &|| false).unwrap();
            same(native.after_bytes(), &after);
        }
    }
}

#[test]
fn nested_arguments_and_closed_eval_cannot_expand_native_sources() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(&seed, "Items", &[Some(23)]);
    for expression in [
        "CLng(IIf(False,DLookup('CopiedScore','Items'),1))",
        "CLng(Abs((SELECT Max(CopiedScore) FROM Items)))",
        "CLng(IIf(False,Forms!Items!CopiedScore,1))",
        "CLng(VBA.Abs(1))",
        "CLng(UserCode(1))",
        "CLng(NPV(0,Array(1,DLookup('CopiedScore','Items'))))",
    ] {
        let sql = format!("SELECT {expression} AS CopiedScore INTO Archive FROM Items AS i");
        assert!(
            matches!(prepare(&before, &sql), Err(NativeMakeTableError::Shape)),
            "{expression}"
        );
    }
    for (expression, expected) in [
        ("CLng(Eval('[CopiedScore]'))", EvalError::UnboundField),
        (
            "CLng(Eval('IIf(False,[CopiedScore],1)'))",
            EvalError::UnboundField,
        ),
        ("CLng(Eval('DSum(1,2)'))", EvalError::UnsupportedExpression),
        ("CLng(Eval('1; SELECT 2'))", EvalError::DynamicSyntax),
    ] {
        let sql = format!("SELECT {expression} AS CopiedScore INTO Archive FROM Items AS i");
        assert!(
            matches!(
                prepare(&before, &sql),
                Err(NativeMakeTableError::Query(MakeTableError::Query(
                    QueryError::Evaluation(error)
                ))) if error == expected
            ),
            "{expression}"
        );
    }
}
