use super::*;

#[test]
fn raw_byte_observations_survive_named_and_derived_expansion_until_strict_publication() {
    let source = native(fixtures::database(
        &[("Label", "IIf([Id]=1,'😀','é') & ''")],
        &[1, 2],
    ));
    let views = RecomputedNativeSource::new(
        &source,
        &[CalculationView::new("Fresh", "Items", &["Id", "Label"])],
        ParseLimits::default(),
    )
    .unwrap();

    for declared in [false, true] {
        let actual = rows(
            &views,
            "SELECT Id,LenB(B),AscB(B) FROM (SELECT Id,LeftB(Label,1) AS B FROM Fresh) Q ORDER BY Id",
            declared,
        )
        .unwrap();
        // UTF-16LE begins 3D D8 00 DE for 😀 and E9 00 for é.
        let expected = [
            vec![Value::Int32(1), Value::Int32(1), Value::Int16(61)],
            vec![Value::Int32(2), Value::Int32(1), Value::Int16(233)],
        ];
        assert_eq!(actual, expected);
        assert!(rows(&views, "SELECT LeftB(Label,1) FROM Fresh", declared).is_err());

        let whole = rows(&views, "SELECT Label FROM Fresh ORDER BY Id", declared).unwrap();
        assert_eq!(
            whole,
            [
                vec![Value::Text("😀".into())],
                vec![Value::Text("é".into())]
            ]
        );
    }
}

struct FoldAscii;

impl Collation for FoldAscii {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase()))
    }
}

#[test]
fn reused_named_view_plans_use_the_current_explicit_text_provider() {
    let source = native(fixtures::database(
        &[("Label", "IIf([Id]=1,'a','A')")],
        &[1, 2],
    ));
    let views = RecomputedNativeSource::new(
        &source,
        &[CalculationView::new("Fresh", "Items", &["Label"])],
        ParseLimits::default(),
    )
    .unwrap();
    let syntax = parse_query(
        "SELECT Label,Count(*) FROM Fresh GROUP BY Label ORDER BY Label",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &views).unwrap();

    for declared in [false, true] {
        let ordinal = plan.execute(&views, &[], options(declared)).unwrap();
        assert_eq!(
            ordinal.rows(),
            [
                vec![Value::Text("A".into()), Value::Int64(1)],
                vec![Value::Text("a".into()), Value::Int64(1)],
            ]
        );
        let folded = plan
            .execute(
                &views,
                &[],
                ExecutionOptions {
                    collation: Some(&FoldAscii),
                    ..options(declared)
                },
            )
            .unwrap();
        assert_eq!(
            folded.rows(),
            [vec![Value::Text("a".into()), Value::Int64(2)]]
        );
    }
}

#[test]
fn composition_keeps_selected_formula_errors_eager_and_does_not_fall_back_to_caches() {
    let source = native(fixtures::database(
        &[("Bad", "IIf([Id]=1,[Id],1/([Id]-2))")],
        &[1, 2, 3],
    ));
    let views = RecomputedNativeSource::new(
        &source,
        &[CalculationView::new("Fresh", "Items", &["Id", "Bad"])],
        ParseLimits::default(),
    )
    .unwrap();

    for sql in [
        "SELECT Q.Id FROM (SELECT Id,Bad FROM Fresh) Q WHERE False",
        "SELECT Count(*) FROM Fresh",
        "SELECT I.Id FROM Items I INNER JOIN Fresh F ON I.Id=F.Id WHERE I.Id=1",
        "SELECT DLookup('Bad','Fresh','Id=1') FROM Items WHERE Id=1",
    ] {
        for declared in [false, true] {
            let error = rows(&views, sql, declared).unwrap_err();
            assert!(
                matches!(error, QueryError::Evaluation(EvalError::DivisionByZero)),
                "{sql}: {error}"
            );
        }
    }
}
