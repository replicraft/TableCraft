mod support;

use support::run;

#[test]
fn string_addition_retains_units_but_keeps_numeric_and_null_dispatch() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT ChrW(55357)+ChrW(56832) AS Face,ChrW(55296)+NULL AS N,CStr(LeftB('12',4)+1) AS Amount",
            "--number-format",
            "invariant",
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
        assert_eq!(row.iter().collect::<Vec<_>>(), ["\"😀\"", "NULL", "\"13\""]);

        for expression in [
            "ChrW(55296)+'x'",
            "LenB(ChrB(65)+ChrB(0))",
            "ChrW(55296)+CVErr(1)",
            "(ChrW(55296)+NULL)+(1/0)",
            "LeftB('12',4)+1",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
