use super::*;

fn text_pivot(headings: &str) -> String {
    FIXED.replace(
        "PIVOT Quarter IN (2,1,4)",
        &format!("PIVOT IIf(Quarter=1,'1',IIf(Quarter=2,'2','4')) IN ({headings})"),
    )
}

#[test]
fn pure_text_headings_prepare_without_scans_and_match_literal_named_grids() {
    for headings in [
        "Left('2x',1),Right('x1',1),Mid('x4y',2,1)",
        "LTrim(' 2'),RTrim('1 '),Trim(' 4 ')",
        "StrReverse('2'),ChrW(49),ChrW(52)",
        "IIf(#1899-12-31#,'2','x'),Choose(#1899-12-31#,'1'),Switch(#1899-12-31#,'4')",
        "LeftB('2x',2),RightB('x1',2),MidB('x4y',3,2)",
    ] {
        let computed = text_pivot(headings);
        let baseline = text_pivot("'2','1','4'");
        let source = Views::new(&[("Piv", &computed), ("Baseline", &baseline)]);
        let syntax = parse_query(
            "SELECT Region,[2],[1],[4] FROM Piv ORDER BY Region",
            Default::default(),
        )
        .unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0, "{headings}");
        assert_eq!(plan.output_names().unwrap(), ["Region", "2", "1", "4"]);

        let actual = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&Ordinal),
                    ..Default::default()
                },
            )
            .unwrap();
        let expected = source
            .run(
                "SELECT Region,[2],[1],[4] FROM Baseline ORDER BY Region",
                &[],
            )
            .unwrap();
        assert_eq!(actual.columns(), expected.columns(), "{headings}");
        assert_eq!(actual.rows(), expected.rows(), "{headings}");
    }
}

#[test]
fn raw_pair_construction_publishes_only_the_complete_final_heading() {
    for heading in [
        "ChrW(55357)&ChrW(56832)",
        "ChrW(55357)+ChrW(56832)",
        "CVar(ChrW(55357))&CVar(ChrW(56832))",
        "IIf(True,ChrW(55357),'x')&IIf(True,ChrW(56832),'x')",
        "Nz(NULL,ChrW(55357))&Nz(NULL,ChrW(56832))",
        "CVar(IIf(True,Nz(NULL,ChrW(55357)+ChrW(56832)),'x'))",
        "Choose(2,'x',ChrW(55357))&Switch(True,ChrW(56832))",
    ] {
        let computed = text_pivot(&format!("{heading},'1','4'"));
        let baseline = text_pivot("'😀','1','4'");
        let source = Views::new(&[("Piv", &computed), ("Baseline", &baseline)]);
        let syntax = parse_query("SELECT * FROM Piv ORDER BY Region", Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0, "{heading}");
        assert_eq!(
            plan.output_names().unwrap(),
            ["Region", "Total", "😀", "1", "4"]
        );

        let actual = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&Ordinal),
                    ..Default::default()
                },
            )
            .unwrap();
        let expected = source
            .run("SELECT * FROM Baseline ORDER BY Region", &[])
            .unwrap();
        assert_eq!(actual.columns(), expected.columns(), "{heading}");
        assert_eq!(actual.rows(), expected.rows(), "{heading}");
    }
}

#[test]
fn raw_unit_and_byte_observers_prepare_numeric_headings_without_unicode_loss() {
    for headings in [
        "LenB(Left('😀',1)),Len(ChrW(55296)),LenB(ChrW(65)&ChrW(66))",
        "Len('😀'),AscB(LeftB('ā',1)),LenB('ab')",
        "LenB(MidB('abc',2,2)),LenB(ChrB(65)),Len('abcd')",
        "LenB(Left('😀',1)),Len(IIf(True,ChrW(55296),'x')),Len('abcd')",
        "LenB(Left('😀',1)),Len(Nz(NULL,ChrW(55296))),Len('abcd')",
        "LenB(Left('😀',1)),Len(Choose(2,'x',ChrW(55296))),Len('abcd')",
        "LenB(Left('😀',1)),Len(Switch(False,'x',True,ChrW(55296))),Len('abcd')",
    ] {
        let computed = FIXED.replace("IN (2,1,4)", &format!("IN ({headings})"));
        let source = Views::new(&[("Piv", &computed), ("Baseline", FIXED)]);
        let syntax = parse_query("SELECT * FROM Piv ORDER BY Region", Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0, "{headings}");
        let actual = plan.execute(&source, &[], Default::default()).unwrap();
        let expected = source
            .run("SELECT * FROM Baseline ORDER BY Region", &[])
            .unwrap();
        assert_eq!(actual.columns(), expected.columns(), "{headings}");
        assert_eq!(actual.rows(), expected.rows(), "{headings}");
    }
}

#[test]
fn text_heading_schemas_compose_through_named_derived_and_union_sources() {
    let computed = text_pivot("Trim('2 '),ChrW(49),Right('x4',1)");
    let source = Views::new(&[
        ("Piv", &computed),
        ("Wrapped", "SELECT Region,[2],[1],[4] FROM Piv"),
        (
            "Combined",
            "SELECT * FROM Wrapped UNION ALL SELECT * FROM Wrapped",
        ),
    ]);
    let syntax = parse_query(
        "SELECT * FROM (SELECT * FROM Combined) Q ORDER BY Region",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let actual = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Ordinal),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(actual.columns(), ["Region", "2", "1", "4"]);
    assert_eq!(actual.rows().len(), 6);
    assert_eq!(actual.rows()[0], actual.rows()[1]);
    assert_eq!(actual.rows()[2], actual.rows()[3]);
    assert_eq!(actual.rows()[4], actual.rows()[5]);
}

#[test]
fn pure_text_heading_names_keep_collision_and_final_validation_rules() {
    for (headings, expected) in [
        ("Trim('Q1 '),'q1'", QueryError::AmbiguousName),
        ("StrReverse('noigeR')", QueryError::AmbiguousName),
        ("Trim(' ')", QueryError::Unsupported),
        ("ChrW(10)", QueryError::Unsupported),
        ("ChrW(55296)", QueryError::Unsupported),
        ("ChrB(65)", QueryError::Unsupported),
        ("CVar(ChrW(55296))", QueryError::Unsupported),
        ("IIf(True,ChrW(55296),'x')", QueryError::Unsupported),
        ("Nz(NULL,ChrB(65))", QueryError::Unsupported),
        ("ChrW(56832)&ChrW(55357)", QueryError::Unsupported),
    ] {
        let computed = text_pivot(headings);
        let source = Views::new(&[("Piv", &computed)]);
        let syntax = parse_query("SELECT * FROM Piv", Default::default()).unwrap();
        assert_eq!(
            prepare(syntax.query(), &source).err(),
            Some(expected),
            "{headings}"
        );
        assert_eq!(source.scans.get(), 0);
    }
}

#[test]
fn excluded_contexts_parameters_and_eager_errors_never_hide_in_constant_calls() {
    for headings in [
        "Len(P)",
        "Len(IIf(True,'x',P))",
        "Left('x','1')",
        "Len(123)",
        "UCase('q1')",
        "Chr(65)",
        "CStr(1)",
        "StrComp('a','b',0)",
        "Len(IIf(True,'x',CurrentUser()))",
    ] {
        let computed = format!("PARAMETERS P TEXT; {}", text_pivot(headings));
        let source = Views::new(&[("Piv", &computed)]);
        let syntax = parse_query("SELECT * FROM Piv", Default::default()).unwrap();
        assert_eq!(
            prepare(syntax.query(), &source).err(),
            Some(QueryError::Unsupported),
            "{headings}"
        );
        assert_eq!(source.scans.get(), 0);
    }
    for headings in [
        "Len(IIf(True,'x',Left('x',-1)))",
        "Len(Choose(1,'x',Mid('x',0)))",
    ] {
        let computed = text_pivot(headings);
        let mut source = Views::new(&[("Piv", &computed)]);
        let columns = source.data.columns("Sales").unwrap().to_vec();
        source.data = MemorySource::new(vec![
            MemoryTable::new("Sales".into(), columns, Vec::new()).unwrap(),
        ])
        .unwrap();
        let syntax = parse_query("SELECT * FROM Piv", Default::default()).unwrap();
        assert_eq!(
            prepare(syntax.query(), &source).err(),
            Some(QueryError::Evaluation(EvalError::Domain)),
            "{headings}"
        );
        assert_eq!(source.scans.get(), 0);
    }
}
