use super::*;

#[test]
fn converted_numeric_union_results_do_not_retain_unselected_field_policies() {
    let legacy = Fixture::new(None);
    let modern = Fixture::column_order([9, 4, 0, 1]);
    for set in ["UNION", "UNION ALL"] {
        let arms = format!(
            "SELECT NBR_VALUE AS V FROM ROULETTE_TB WHERE ORDR_NBR=1 \
             {set} SELECT IIf(True,3,NBR_VALUE) FROM [remote].[ROULETTE_TB] WHERE ORDR_NBR=1"
        );
        let sql = format!("SELECT Asc(V) AS Code,VarType(V) AS Kind FROM ({arms}) Q ORDER BY V");
        let output = legacy.query(
            &sql,
            &[
                "--external",
                "remote",
                modern.path(),
                "--number-format",
                "invariant",
            ],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"Code\tKind\n50\t8\n51\t8\n");
    }
}

#[test]
fn actual_text_and_null_union_policies_still_conflict() {
    let legacy = Fixture::new(None);
    let modern = Fixture::column_order([9, 4, 0, 1]);
    for value in ["NBR_VALUE", "IIf(True,NULL,NBR_VALUE)"] {
        let sql = format!(
            "SELECT NBR_VALUE AS V FROM ROULETTE_TB WHERE ORDR_NBR IN (1,2) \
             UNION ALL SELECT {value} FROM [remote].[ROULETTE_TB] \
             WHERE ORDR_NBR=1 ORDER BY V"
        );
        let output = legacy.query(&sql, &["--external", "remote", modern.path()]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{value}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(output.stdout.is_empty());
    }
}
