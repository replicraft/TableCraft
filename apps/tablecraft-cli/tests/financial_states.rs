mod support;
use support::run;

#[test]
fn financial_error_data_retains_eager_and_conversion_order_with_buffered_failures() {
    for mode in ["eval", "run"] {
        for (expression, error) in [
            ("SLN(CVErr(1),1/0,10)", "division by zero"),
            ("Pmt(CVErr(1),10,1/0)", "division by zero"),
            ("SLN(CVErr(1),0,10)", "type mismatch"),
            ("SLN(NULL,CVErr(1),10)", "NULL"),
            ("EuroConvert(1,'USD',CVErr(1))", "domain"),
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains(error));
        }
        let result = run(&["query", mode, "SELECT SLN(CLng(CVErr(1)),0,1) AS N"]);
        assert!(result.status.success());
        assert_eq!(result.stdout, b"N\n1\n");
    }
}
