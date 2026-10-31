//! Reuse genuine owned envelope constructors; qualify readers before refusals.
#[path = "../../../../../crates/backends/tablecraft-access/tests/support/deployment.rs"]
mod deployment_fixture;
#[path = "../../../../../crates/engines/tablecraft-ace/tests/support/agile.rs"]
mod encrypted_fixture;

use super::super::{CASES, inspect, physical_roles, qualify_view};
use std::{
    fs,
    io::{Cursor, Read},
    process::{Command, Stdio},
};
use tablecraft_access::AccessInput;
use tablecraft_ace::{EncryptedSource, check_table, inspect_header, read_table};
use tablecraft_model::CatalogScope;

const PASSWORD: &str = "public fixture password";

struct Envelopes {
    before: Vec<u8>,
    encrypted: Vec<u8>,
    stored: Vec<u8>,
    compressed: Vec<u8>,
    bag: Vec<u8>,
}

fn envelopes() -> Envelopes {
    let (_, body, _) = CASES[0];
    let (before, _, bag) = physical_roles::source(6, body);
    let encrypted = encrypted_fixture::encrypted_image(&before, 6, PASSWORD);
    let stored = deployment_fixture::cabinet(&before, false);
    let compressed = deployment_fixture::cabinet(&before, true);
    Envelopes {
        before,
        encrypted,
        stored,
        compressed,
        bag,
    }
}

#[test]
fn qualify_before_change_envelopes_before_encrypted_or_deployment_command_calls() {
    let inputs = envelopes();
    qualify_view(&inputs.before, 20, &inputs.bag);
    let expected = read_table(
        &mut Cursor::new(&inputs.before),
        CatalogScope::new(7),
        "Items",
        &mut inspect::budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        inspect_header(
            &mut Cursor::new(&inputs.encrypted),
            &mut inspect::budget(),
            &|| false
        )
        .unwrap()
        .encoded_pages()
    );
    let mut decoded = EncryptedSource::open(
        Cursor::new(&inputs.encrypted),
        Some(PASSWORD),
        &mut inspect::budget(),
        &|| false,
    )
    .unwrap();
    let actual = read_table(
        &mut decoded,
        CatalogScope::new(7),
        "Items",
        &mut inspect::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(actual.rows(), expected.rows());
    let checked = check_table(
        &mut decoded,
        CatalogScope::new(7),
        "Items",
        &mut inspect::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert!(checked.indexes().is_empty());
    for bytes in [&inputs.stored, &inputs.compressed] {
        let mut input =
            AccessInput::open(Cursor::new(bytes), &mut inspect::budget(), &|| false).unwrap();
        assert!(input.is_deployment());
        let mut payload = Vec::new();
        input.read_to_end(&mut payload).unwrap();
        super::assert_image(&payload, &inputs.before);
        qualify_view(&payload, 20, &inputs.bag);
    }
}

#[test]
fn before_change_encrypted_and_deployment_refusals_preserve_all_inputs() {
    let inputs = envelopes();
    for required in [false, true] {
        if required && !super::QUALIFIED_REQUIRED {
            continue;
        }
        for (bytes, password, message) in [
            (&inputs.encrypted, None, "encrypted rewrite is unsupported"),
            (
                &inputs.encrypted,
                Some(PASSWORD),
                "encrypted rewrite is unsupported",
            ),
            (
                &inputs.stored,
                None,
                "deployment packages cannot be BeforeChange copy sources",
            ),
            (
                &inputs.compressed,
                None,
                "deployment packages cannot be BeforeChange copy sources",
            ),
        ] {
            let root = tempfile::tempdir().unwrap();
            let input = root.path().join("source.accdb");
            let output = root.path().join("copy.accdb");
            fs::write(&input, bytes).unwrap();
            let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
            command
                .args(["query", "preview-before-change", CASES[0].0])
                .arg("--database")
                .arg(&input)
                .arg("--output")
                .arg(&output)
                .args(["--guard-property", "3", "StoredGuard"])
                .stdin(Stdio::null());
            if required {
                command.arg("--durable-output");
            }
            if let Some(password) = password {
                command.args(["--password", password]);
            }
            let result = command.output().unwrap();
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&result.stderr).contains(message),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            super::assert_image(&fs::read(&input).unwrap(), bytes);
            assert!(!output.exists());
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        }
    }
}
