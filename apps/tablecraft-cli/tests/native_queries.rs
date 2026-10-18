mod support;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use support::run;
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    path: PathBuf,
    bytes: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        let bytes = std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/examples/Access_Example_VBA.accdb"),
        )
        .unwrap();
        let path = std::env::temp_dir().join(format!(
            "tablecraft-native-query-{}-{}.accdb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, &bytes).unwrap();
        Self { path, bytes }
    }
    fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }
    fn retagged(code: u8) -> Self {
        // Shared-layout regression copy; actual newer-family fixture evidence
        // comes from the original recipes in docs/testing/compare_ace16.py.
        let mut fixture = Self::new();
        fixture.bytes[20] = code;
        std::fs::write(&fixture.path, &fixture.bytes).unwrap();
        fixture
    }
    fn query(&self, sql: &str, extra: &[&str]) -> std::process::Output {
        let mut args = vec!["query", "run", "--database", self.path(), sql];
        args.extend(extra);
        run(&args)
    }
}

#[test]
fn ace16_family_cli_reads_use_native_sources_but_keep_writes_blocked() {
    for code in [5, 6] {
        let fixture = Fixture::retagged(code);
        let result = fixture.query(
            "SELECT ORDR_NBR FROM ROULETTE_TB WHERE ORDR_NBR <= 3 ORDER BY ORDR_NBR",
            &["--index", "ROULETTE_TB", "ORDR_NBR"],
        );
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"ORDR_NBR\n1\n2\n3\n");
        let output = fixture.path.with_extension("updated.accdb");
        let result = run(&[
            "query",
            "update",
            "UPDATE ROULETTE_TB SET ORDR_NBR=1",
            "--database",
            fixture.path(),
            "--output",
            output.to_str().unwrap(),
        ]);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(std::fs::read(&self.path).unwrap(), self.bytes);
        let _ = std::fs::remove_file(&self.path);
    }
}
#[test]
fn native_query_runs_filter_order_top_parameters_and_aggregate_over_real_rows() {
    let fixture = Fixture::new();
    let result = fixture.query(
        "SELECT TOP 5 ORDR_NBR, NBR_VALUE, COLOR_ID FROM ROULETTE_TB ORDER BY ORDR_NBR",
        &[],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    assert_eq!(result.stdout,b"ORDR_NBR\tNBR_VALUE\tCOLOR_ID\n1\t\"2\"\t0\n2\t\"0\"\t65280\n3\t\"28\"\t0\n4\t\"9\"\t255\n5\t\"26\"\t0\n");
    let result=fixture.query("PARAMETERS Floor INTEGER; SELECT TOP 2 ORDR_NBR FROM ROULETTE_TB WHERE ORDR_NBR > Floor ORDER BY ORDR_NBR",&["--param","Floor=long:3"]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"ORDR_NBR\n4\n5\n");
    let result = fixture.query("SELECT Count(*) AS N FROM TAG_NME_TB", &[]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"N\n933\n");
}
#[test]
fn native_sources_support_joins_empty_tables_and_explicit_text_collation() {
    let fixture = Fixture::new();
    let result=fixture.query("SELECT a.ORDR_NBR FROM ROULETTE_TB AS a INNER JOIN ROULETTE_TB AS b ON a.ORDR_NBR=b.ORDR_NBR WHERE a.ORDR_NBR <= 3 ORDER BY a.ORDR_NBR",&[]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"ORDR_NBR\n1\n2\n3\n");
    let result = fixture.query("SELECT * FROM tblContacts", &[]);
    assert!(result.status.success());
    assert_eq!(
        result.stdout,
        b"ID\tFirstName\tLastName\tAddress\tCity\tState\tZip_Code\n"
    );
    let result = fixture.query(
        "SELECT ORDR_NBR FROM ROULETTE_TB WHERE NBR_VALUE='0'",
        &["--collation", "ordinal"],
    );
    assert!(result.status.success());
    assert_eq!(result.stdout, b"ORDR_NBR\n2\n");
}
#[test]
fn native_failures_do_not_publish_partial_results_or_sensitive_paths_and_names() {
    let fixture = Fixture::new();
    for sql in [
        "SELECT * FROM MSysResources",
        "SELECT * FROM [private-secret-name]",
        "SELECT * FROM AutoKeys",
        "DELETE FROM ROULETTE_TB",
        "SELECT * FROM ROULETTE_TB WHERE NBR_VALUE='secret-payload'",
    ] {
        let result = fixture.query(sql, &[]);
        assert_eq!(result.status.code(), Some(2), "{sql}");
        assert!(result.stdout.is_empty());
        let error = String::from_utf8(result.stderr).unwrap();
        for private in [fixture.path(), "private-secret-name", "secret-payload"] {
            assert!(!error.contains(private));
        }
    }
    let result = run(&[
        "query",
        "run",
        "--database",
        "missing-private-db-secret.accdb",
        "SELECT 1",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&result.stderr).contains("private-db-secret"));
}

#[test]
fn native_dates_project_filter_sort_and_accept_explicit_iso_parameters() {
    let fixture = Fixture::new();
    let result = fixture.query(
        "SELECT Count(*) AS N FROM CUM_VAL_TB WHERE CUM_VAL_DATE >= #0100-01-01#",
        &[],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"N\n59\n");
    let result=fixture.query("PARAMETERS Start DATETIME; SELECT Count(*) AS N FROM CUM_VAL_TB WHERE CUM_VAL_DATE >= Start",&["--param","Start=date:0100-01-01"]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"N\n59\n");
    let result = fixture.query(
        "SELECT TOP 1 CUM_VAL_DATE FROM CUM_VAL_TB ORDER BY CUM_VAL_DATE",
        &[],
    );
    assert!(result.status.success());
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(text.starts_with("CUM_VAL_DATE\n#"));
    assert!(text.ends_with("00:00:00.000000000#\n"));
    let result = fixture.query(
        "SELECT * FROM tblContacts WHERE #2026-02-30# = #2026-02-30#",
        &[],
    );
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}

#[test]
fn long_text_is_read_as_inert_data_including_ribbon_xml() {
    let fixture = Fixture::new();
    let output = fixture.query("SELECT Count(*) AS N FROM CMD_LINE_TB", &[]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"N\n14\n");
    let output = fixture.query("SELECT RibbonName, RibbonXML FROM USysRibbons", &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("customUI"));
    assert_eq!(text.lines().count(), 3);
}

#[test]
fn native_binary_output_is_hex_and_guid_tables_are_bound_even_when_empty() {
    let fixture = Fixture::new();
    let result = fixture.query("SELECT TOP 1 SID FROM MSysACEs", &[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(text.starts_with("SID\n0x"));
    assert!(
        text.lines().nth(1).unwrap()[2..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    );
    let result = fixture.query("SELECT * FROM MSysComplexType_GUID", &[]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"Value\n");
    let result = fixture.query(
        "PARAMETERS P GUID; SELECT P AS G",
        &["--param", "P=guid:6B29FC40-CA47-1067-B31D-00DD010662DA"],
    );
    assert!(result.status.success());
    assert_eq!(
        result.stdout,
        b"G\n{6b29fc40-ca47-1067-b31d-00dd010662da}\n"
    );
}

#[test]
fn native_catalog_table_can_be_queried_as_inert_scalar_data() {
    let fixture = Fixture::new();
    let result = fixture.query("SELECT Count(*) AS N FROM MSysObjects", &[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"N\n146\n");
}
