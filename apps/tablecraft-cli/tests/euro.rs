mod support;

use support::run;

#[test]
fn euro_queries_execute_optional_slots_and_buffer_errors_in_both_commands() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT EuroConvert(1.2,'DEM','EUR'),EuroConvert(1.47,'FRF','DEM'),EuroConvert(1000,'FRF','DEM',True,3),EuroConvert(1.2,'DEM','EUR',,)",
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
        let actual = row
            .iter()
            .map(|text| text.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        for (actual, expected) in actual.iter().zip([0.61, 0.44, 152.0 * 1.95583, 0.61]) {
            assert!((actual - expected).abs() < 1e-12);
        }
        assert_eq!(actual.len(), 4);
        for expression in [
            "EuroConvert(1,'USD','EUR')",
            "EuroConvert(1,'DEM','EUR',Null)",
            "EuroConvert(1,'DEM','EUR',True,2)",
            "EuroConvert(Null,'DEM','EUR',1/0)",
            "EuroConvert(*)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}

#[test]
fn financial_numeric_text_uses_explicit_policy_and_keeps_failures_buffered() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Pmt('0','10','1000') AS P,EuroConvert('1,5','EUR','EUR','1','3,5') AS E",
            "--number-format",
            "de-de",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"P\tE\n-100\t1.5\n");
        let failure = run(&["query", mode, "SELECT 1,Pmt('0','10','1000')"]);
        assert_eq!(failure.status.code(), Some(2));
        assert!(failure.stdout.is_empty());
        assert!(String::from_utf8_lossy(&failure.stderr).contains("number-format"));
    }
}
