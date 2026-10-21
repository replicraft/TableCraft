mod support;
use support::run;
fn rows(bytes: &[u8]) -> Vec<Vec<String>> {
    csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_reader(bytes)
        .records()
        .map(|record| record.unwrap().iter().map(str::to_owned).collect())
        .collect()
}
#[test]
fn fixed_clock_projects_exact_components_and_truncates_before_midnight() {
    for mode in ["eval", "run"] {
        let result = run(&[
            "query",
            mode,
            "SELECT Year(Now()) AS Y,Month(Date()) AS M,Day(Date()) AS D,Hour(Time()) AS H,Minute(Now()) AS N,Second(Time()) AS S",
            "--clock",
            "2024-02-29T23:59:59.999999999",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            rows(&result.stdout),
            [
                vec!["Y", "M", "D", "H", "N", "S"],
                vec!["2024", "2", "29", "23", "59", "59"]
            ]
        );
    }
}
#[test]
fn invalid_calls_and_readings_publish_nothing_and_inert_parse_has_no_clock_flag() {
    for mode in ["eval", "run"] {
        for expression in ["Now(1/0)", "Date(*)", "Time(,)", "VBA.Now()"] {
            let result = run(&[
                "query",
                mode,
                &format!("SELECT 1,{expression}"),
                "--clock",
                "2026-10-20T12:34:56",
            ]);
            assert!(!result.status.success(), "{expression}");
            assert!(result.stdout.is_empty());
        }
        for reading in [
            "0001-01-01T00:00:00",
            "2026-02-29T00:00:00",
            "2026-10-20T24:00:00",
            "2026-10-20T12:00:00Z",
            "2026-10-20T12:00:00+01:00",
            "2026-10-20T12:00:00-01:00",
        ] {
            let result = run(&["query", mode, "SELECT 1", "--clock", reading]);
            assert!(!result.status.success(), "{reading}");
            assert!(result.stdout.is_empty());
        }
    }
    let result = run(&[
        "query",
        "parse",
        "SELECT Now()",
        "--clock",
        "2026-10-20T12:00:00",
    ]);
    assert!(!result.status.success());
}
#[test]
fn local_system_clock_is_lazy_and_fixed_readings_bypass_invalid_timezone() {
    use std::process::Command;
    for mode in ["eval", "run"] {
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .env("TZ", "TableCraft/NoSuchZone")
            .args(["query", mode, "SELECT 1"])
            .output()
            .unwrap();
        assert!(result.status.success());
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .env("TZ", "TableCraft/NoSuchZone")
            .args(["query", mode, "SELECT 1,Now()"])
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .env("TZ", "TableCraft/NoSuchZone")
            .args([
                "query",
                mode,
                "SELECT Year(Now())",
                "--clock",
                "2026-10-20T12:00:00",
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 86400;
    let result = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
        .env("TZ", "UTC")
        .args(["query", "eval", "SELECT DateDiff('d',#1970-01-01#,Date())"])
        .output()
        .unwrap();
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 86400;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let day = rows(&result.stdout)[1][0].parse::<u64>().unwrap();
    assert!((before..=after).contains(&day));
}
