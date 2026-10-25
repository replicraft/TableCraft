#[path = "../../../crates/engines/tablecraft-ace/tests/support/agile.rs"]
mod crypto;
#[path = "../../../crates/backends/tablecraft-access/tests/support/deployment.rs"]
mod fixture;
mod support;
use std::{fs, path::Path};
use support::run;

#[test]
fn packaged_plain_and_encrypted_databases_feed_the_same_read_commands() {
    let original = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    for encrypted in [false, true] {
        let database = if encrypted {
            crypto::encrypted_image(&original, 5, "public test password")
        } else {
            original.clone()
        };
        for compressed in [false, true] {
            let bytes = fixture::cabinet(&database, compressed);
            let path = directory
                .path()
                .join(format!("example-{encrypted}-{compressed}.accdc"));
            fs::write(&path, &bytes).unwrap();
            let path = path.to_str().unwrap();
            let inspection = run(&["inspect", path]);
            assert!(inspection.status.success(), "{:?}", inspection.stderr);
            assert!(
                String::from_utf8_lossy(&inspection.stdout)
                    .contains("Package signature\tNot verified")
            );
            let tables = run(&["tables", path, "--count", "-p", "public test password"]);
            assert!(
                tables.status.success(),
                "encrypted={encrypted}, compressed={compressed}: {}",
                String::from_utf8_lossy(&tables.stderr),
            );
            assert!(String::from_utf8_lossy(&tables.stdout).contains("ROULETTE_TB\tUser\t38"));
            let query = run(&[
                "query",
                "run",
                "SELECT TOP 2 * FROM ROULETTE_TB",
                "--database",
                path,
                "-p",
                "public test password",
            ]);
            let rows = run(&[
                "rows",
                path,
                "ROULETTE_TB",
                "--limit",
                "2",
                "-p",
                "public test password",
            ]);
            assert!(query.status.success(), "{:?}", query.stderr);
            assert!(rows.status.success(), "{:?}", rows.stderr);
            assert_eq!(query.stdout, rows.stdout);
            assert_eq!(fs::read(path).unwrap(), bytes);
            if encrypted {
                for flags in [&[][..], &["-p", "wrong"][..]] {
                    let args = [&["rows", path, "ROULETTE_TB"][..], flags].concat();
                    let result = run(&args);
                    assert_eq!(result.status.code(), Some(2));
                    assert!(result.stdout.is_empty());
                }
            }
        }
    }
}

#[test]
fn inspection_and_inventory_keep_explicit_container_allowances_bounded() {
    let original = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let package = fixture::cabinet(&original, true);
    let path = directory.path().join("allowance.accdc");
    fs::write(&path, &package).unwrap();
    let path = path.to_str().unwrap();

    for command in ["inspect", "tables"] {
        let admitted = run(&[command, path, "--native-bytes", "33554432"]);
        assert!(admitted.status.success(), "{:?}", admitted.stderr);

        let exhausted = run(&[command, path, "--native-bytes", "1"]);
        assert_eq!(exhausted.status.code(), Some(2));
        assert!(exhausted.stdout.is_empty());

        for amount in ["0", "268435457"] {
            let refused = run(&[command, "missing.accdc", "--native-bytes", amount]);
            assert_eq!(refused.status.code(), Some(2));
            assert!(refused.stdout.is_empty());
            assert!(String::from_utf8_lossy(&refused.stderr).contains("invalid value"));
        }
    }
    let rows = run(&[
        "rows",
        path,
        "ROULETTE_TB",
        "--limit",
        "1",
        "--native-bytes",
        "33554432",
    ]);
    assert!(rows.status.success(), "{:?}", rows.stderr);
    let exhausted = run(&["rows", path, "ROULETTE_TB", "--native-bytes", "1"]);
    assert_eq!(exhausted.status.code(), Some(2));
    assert!(exhausted.stdout.is_empty());

    assert_eq!(fs::read(path).unwrap(), package);
}
