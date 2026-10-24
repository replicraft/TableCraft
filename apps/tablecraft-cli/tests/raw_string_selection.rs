mod support;

use support::run;

#[test]
fn lossless_selectors_work_in_both_hosts_and_buffer_failures() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT LenB(Nz(ChrW(55296),ChrB(65))) AS N,LenB(Choose(2,ChrW(55296),ChrB(65))) AS C,VarType(Switch(True,ChrW(55296))) AS S,IsNull(Switch(True,NULL,True,ChrW(55296))) AS Z",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"N\tC\tS\tZ\n2\t1\t8\tTRUE\n");

        for expression in [
            "LenB(Nz(ChrW(55296),1/0))",
            "LenB(Choose(1e100,ChrW(55296),1/0))",
            "Nz('ok',ChrW(65536))",
            "Choose(1,ChrW(55296),ChrB(65))",
        ] {
            let result = run(&["query", mode, &format!("SELECT 1,{expression}")]);
            assert_eq!(result.status.code(), Some(2), "{expression}");
            assert!(result.stdout.is_empty(), "{expression}");
        }
    }
}
