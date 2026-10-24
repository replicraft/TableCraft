use super::*;
use tablecraft_ace::{
    ConstraintViolation, InspectError, Int32Update, RuleCheckStatus, WriteBlocker,
};
use tablecraft_expressions::EvalError;

const PAGE_BYTES: usize = 4096;
const TABLE_PAGE: usize = 10;
const DATA_PAGE: usize = 12;

fn database(rule: &str, text: Option<&str>) -> Vec<u8> {
    let mut bytes = database_with_rule(rule);
    bytes[TABLE_PAGE * PAGE_BYTES..(TABLE_PAGE + 1) * PAGE_BYTES].copy_from_slice(
        &base::definition(&[("Id", 10, 0, 100), ("Score", 4, 0, 4)], 1, 11 << 8),
    );
    let mut row = 2_u16.to_le_bytes().to_vec();
    row.extend(10_i32.to_le_bytes());
    let start = row.len() as u16;
    if let Some(text) = text {
        row.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    }
    row.extend((row.len() as u16).to_le_bytes());
    row.extend(start.to_le_bytes());
    row.extend(1_u16.to_le_bytes());
    row.push(2 | u8::from(text.is_some()));
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(TABLE_PAGE as u32, &[(0, row)]));
    bytes
}

fn table_rule(bytes: &[u8]) -> Result<RuleCheckStatus, InspectError> {
    check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .map(|report| report.table_validation())
}

fn update(bytes: &[u8], value: i32) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    let assignments = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(value),
    )];
    run(
        bytes,
        &[FixedColumnUpdates::new("Items", "Score", &assignments)],
    )
}

#[test]
fn len_and_binary_strcomp_rules_validate_the_complete_int32_candidate() {
    for (rule, valid, invalid) in [
        ("Len([Id]) < [Score]", 5, 4),
        ("StrComp([Id],'zzzz',0) < [Score]", 0, -1),
    ] {
        let original = database(rule, Some("abcd"));
        assert_eq!(table_rule(&original).unwrap(), RuleCheckStatus::Checked);
        let candidate = update(&original, valid).unwrap();
        assert_eq!(candidate.changed_rows(), 1);
        assert_eq!(
            rows(candidate.candidate_bytes(), "Items"),
            [vec![Value::Text("abcd".into()), Value::Int32(valid)]]
        );
        assert_eq!(
            table_rule(candidate.candidate_bytes()).unwrap(),
            RuleCheckStatus::Checked
        );
        assert_eq!(
            update(&original, invalid).err(),
            Some(EditError::Inspect(InspectError::Constraint(
                ConstraintViolation::TableRule
            )))
        );
        assert_eq!(rows(&original, "Items")[0][1], Value::Int32(10));
    }
}

#[test]
fn null_and_unqualified_text_calls_remain_write_blockers() {
    for (rule, text) in [
        ("Len([Id]) < [Score]", None),
        ("StrComp([Id],Null,0) < [Score]", Some("abcd")),
        ("Len([Score]) > 0", Some("abcd")),
        ("StrComp([Id],'abcd',1) < [Score]", Some("abcd")),
    ] {
        let bytes = database(rule, text);
        assert_eq!(
            table_rule(&bytes).unwrap(),
            RuleCheckStatus::Unsupported,
            "{rule}"
        );
        assert_eq!(
            update(&bytes, 11).err(),
            Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage)),
            "{rule}"
        );
    }
}

#[test]
fn admitted_text_calls_do_not_make_validation_logical_operators_short_circuit() {
    for rule in [
        "Len([Id]) > 0 Or 1/0 > 1",
        "StrComp([Id],'abcd',0) = 0 Or 1/0 > 1",
    ] {
        let bytes = database(rule, Some("abcd"));
        let failure =
            || InspectError::Constraint(ConstraintViolation::Evaluation(EvalError::DivisionByZero));
        assert_eq!(table_rule(&bytes).err(), Some(failure()));
        assert_eq!(
            update(&bytes, 11).err(),
            Some(EditError::Inspect(failure()))
        );
    }
}

#[test]
fn ordinary_jet4_int32_plans_recheck_the_same_pure_text_rules_after_editing() {
    for (rule, valid, invalid) in [
        ("Len([Id]) < [Score]", 5, 4),
        ("StrComp([Id],'zzzz',0) < [Score]", 0, -1),
    ] {
        let mut bytes = database(rule, Some("abcd"));
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        bytes[20] = 1;
        assert_eq!(table_rule(&bytes).unwrap(), RuleCheckStatus::Checked);
        let prepare = |value| {
            capture(&bytes).plan_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(0, 10, value)],
                &mut budget(),
                &|| false,
            )
        };
        let plan = prepare(valid).unwrap();
        assert_eq!(plan.changed_rows(), 1);
        assert_eq!(
            table_rule(plan.after_bytes()).unwrap(),
            RuleCheckStatus::Checked
        );
        assert_eq!(rows(plan.after_bytes(), "Items")[0][1], Value::Int32(valid));
        assert_eq!(
            prepare(invalid).err(),
            Some(EditError::Inspect(InspectError::Constraint(
                ConstraintViolation::TableRule
            )))
        );
    }
}
