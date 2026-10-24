mod support;

use support::run;

#[test]
fn numeric_parameters_reach_query_execution_with_their_declared_carriers() {
    let result = run(&[
        "query",
        "run",
        "PARAMETERS D DECIMAL,S SINGLE,F DOUBLE; SELECT D,S,F,TypeName(D),TypeName(S),TypeName(F)",
        "--param",
        "D=decimal:1.2300",
        "--param",
        "S=single:16777217",
        "--param",
        "F=double:16777217",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );

    let records = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(result.stdout.as_slice())
        .records()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].iter().collect::<Vec<_>>(),
        [
            "1.2300",
            "16777216",
            "16777217",
            "\"Decimal\"",
            "\"Single\"",
            "\"Double\"",
        ]
    );
}

#[test]
fn invalid_or_mismatched_numeric_parameters_publish_no_rows() {
    for (declaration, parameter) in [
        ("DECIMAL", "D=decimal:79228162514264337593543950336"),
        ("DECIMAL", "D=decimal:0.00000000000000000000000000001"),
        ("DECIMAL", "D=decimal:1e2"),
        ("DECIMAL", "D=decimal:1,5"),
        ("SINGLE", "D=single:NaN"),
        ("SINGLE", "D=single:1e39"),
        ("DOUBLE", "D=double:inf"),
        ("DOUBLE", "D=double:1e1000"),
        ("SINGLE", "D=double:1.5"),
        ("DOUBLE", "D=single:1.5"),
        ("DECIMAL", "D=double:1.5"),
    ] {
        let sql = format!("PARAMETERS D {declaration}; SELECT 1,D");
        let result = run(&["query", "run", &sql, "--param", parameter]);
        assert_eq!(result.status.code(), Some(2), "{parameter}");
        assert!(result.stdout.is_empty(), "{parameter}");
    }
}

#[test]
fn decimal_parameter_boundaries_reach_output_without_rounding() {
    for text in [
        "79228162514264337593543950335",
        "-79228162514264337593543950335",
        "0.0000000000000000000000000001",
        "0.0000",
    ] {
        let parameter = format!("D=decimal:{text}");
        let result = run(&[
            "query",
            "run",
            "PARAMETERS D DECIMAL; SELECT D AS Exact",
            "--param",
            &parameter,
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            format!("Exact\n{text}\n")
        );
    }
}
