mod support;

use std::path::Path;
use support::run;

const COMPOUND: &str = "SELECT VarType(K) AS T,K AS V,N FROM \
    (SELECT K,Count(*) AS N FROM \
    (SELECT CInt(1) AS Id,IIf(CBool(True),CDec('-0.1'),CCur('-0.1')) AS K \
    UNION ALL SELECT CInt(2) AS Id,IIf(CBool(True),CCur('-0.1'),CDec('-0.1')) AS K \
    UNION ALL SELECT CInt(3) AS Id,IIf(CBool(True),CDbl(-0.1),CDec('-0.1')) AS K) Numbers \
    GROUP BY K) G";

#[test]
fn compound_queries_keep_the_default_and_support_explicit_deeper_binding() {
    let refused = run(&["query", "run", COMPOUND, "--number-format", "invariant"]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(refused.stdout.is_empty());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("structural limit"));

    for declared in [false, true] {
        let mut args = vec![
            "query",
            "run",
            COMPOUND,
            "--binding-depth",
            "32",
            "--number-format",
            "invariant",
        ];
        if declared {
            args.push("--declared-arithmetic");
        }
        let result = run(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"T\tV\tN\n14\t-0.1\t2\n5\t-0.1\t1\n");
    }
}

#[test]
fn invalid_binding_depth_fails_argument_parsing_before_opening_a_database() {
    for depth in ["0", "33", "18446744073709551615", "deep"] {
        let result = run(&[
            "query",
            "run",
            "SELECT 1",
            "--database",
            "missing.accdb",
            "--binding-depth",
            depth,
        ]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&result.stderr)
                .contains("expected binding depth from 1 through 32")
        );
    }
}

#[test]
fn deeper_native_domain_and_progressive_queries_use_the_same_option() {
    let example =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/Access_Example_VBA.accdb");
    let mut expression = "1".to_owned();
    for _ in 0..18 {
        expression = format!("CVar({expression})");
    }
    let domain = format!("SELECT DCount('*','ROULETTE_TB','{expression}=1') AS N");
    let stream = format!("SELECT Count(*) AS N FROM ROULETTE_TB WHERE {expression}=1");
    for (sql, progressive) in [(domain, false), (stream, true)] {
        let mut args = vec![
            "query",
            "run",
            sql.as_str(),
            "--database",
            example.to_str().unwrap(),
        ];
        if progressive {
            args.push("--stream");
        }
        let refused = run(&args);
        assert_eq!(refused.status.code(), Some(2));
        assert!(refused.stdout.is_empty());

        args.extend(["--binding-depth", "32"]);
        let result = run(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"N\n38\n");
    }
}
