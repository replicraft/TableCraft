mod support;

use support::run;

#[test]
fn concatenation_retains_raw_units_and_buffers_final_publication_failures() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT ChrW(55357)&ChrW(56832) AS Face,Len(ChrW(55296)&NULL) AS Units,Nz(NULL)&'x' AS Empty",
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
        assert_eq!(row.iter().collect::<Vec<_>>(), ["\"😀\"", "1", "\"x\""]);

        for expression in [
            "ChrW(55296)&'x'",
            "LenB(ChrB(65)&ChrB(0))",
            "ChrW(55296)&CVErr(1)",
            "ChrW(55296)&(1/0)",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
