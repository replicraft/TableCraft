mod support;
use support::run;

#[test]
fn cli_constructor_components_use_number_policy_without_date_policy() {
    let sql = concat!(
        "SELECT Year(DateSerial('49','2,5','28,5')),",
        "Day(DateSerial(CDate(2024.5),CDate(2.5),CDate(28.5))),",
        "Hour(TimeSerial('1,5','-0,5','0'))"
    );
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            sql,
            "--number-format",
            "de-DE",
            "--two-digit-year-max",
            "2049",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let row = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(result.stdout.as_slice())
            .records()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(row.iter().collect::<Vec<_>>(), ["2049", "28", "2"]);
        for sql in [
            "SELECT 99,DateSerial('2024',1,1)",
            "SELECT 99,TimeSerial('1.5',0,0)",
            "SELECT 99,DateSerial(NULL,1,1/0)",
        ] {
            let result = run(&["query", mode, sql]);
            assert!(!result.status.success(), "{sql}");
            assert!(result.stdout.is_empty());
        }
    }
}

#[test]
fn constructor_error_data_keeps_eager_component_evaluation_and_buffered_failures() {
    for mode in ["eval", "run"] {
        for (expression, error) in [
            ("DateSerial(CVErr(1),1/0,1)", "division by zero"),
            ("TimeSerial(CVErr(1),0,1/0)", "division by zero"),
            ("DateSerial(NULL,CVErr(1),1)", "NULL"),
            ("DateSerial(9999,13,CVErr(1))", "type mismatch"),
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains(error));
        }
        let result = run(&[
            "query",
            mode,
            "SELECT Year(DateSerial(CLng(CVErr(2001)),1,1)) AS Y",
        ]);
        assert!(result.status.success());
        assert_eq!(result.stdout, b"Y\n2001\n");
    }
}
