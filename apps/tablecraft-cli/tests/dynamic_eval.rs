mod support;

use support::run;

#[test]
fn eval_builtin_uses_shared_context_and_keeps_output_buffered() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Eval('15+3'),Eval('Chr$(65)'),Eval('Eval(''CVErr(2001)'')'),Eval('Month(Date())')",
            "--clock",
            "2026-10-21T13:14:15",
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
        assert_eq!(
            row.iter().collect::<Vec<_>>(),
            ["18", "\"A\"", "#ERROR(2001)", "10"]
        );
        let result = run(&["query", mode, "SELECT Eval('Rnd()'),Eval('Rnd(0)')"]);
        assert!(result.status.success());
        let row = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_reader(result.stdout.as_slice())
            .records()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(row.get(0), row.get(1));
        for expression in [
            "Eval('1/0')",
            "Eval('1;SELECT 2')",
            "Eval('(SELECT 1)')",
            "Eval('Shell(''x'')')",
            "Eval('[Secret]')",
            "Eval(Null)",
            "Eval(*)",
            "Eval()",
            "Eval('1',1/0)",
            "VBA.Eval('1')",
            "Eval('IIf(True,1,1/0)')",
            "Eval('15+3'),1/0",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
