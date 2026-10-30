//! Positive readable envelope qualification before explicit rewrite refusals.
#[path = "../../../../crates/backends/tablecraft-access/tests/support/deployment.rs"]
mod deployment_fixture;
#[path = "../../../../crates/engines/tablecraft-ace/tests/support/agile.rs"]
mod encrypted_fixture;

use super::source;
use std::{
    fs,
    io::{Cursor, Read},
    process::{Command, Stdio},
};
use tablecraft_access::AccessInput;
use tablecraft_ace::{EncryptedSource, IndexCheckStatus};
use tablecraft_testkit::access::secondary_index as model;

const PASSWORD: &str = "public test password";

struct Envelopes {
    case: model::Case,
    encrypted: Vec<u8>,
    stored_package: Vec<u8>,
    compressed_package: Vec<u8>,
}

fn envelopes() -> Envelopes {
    let case = model::source(5, model::Population::NullableDuplicate, false);
    let encrypted = encrypted_fixture::encrypted_image(&case.before, 5, PASSWORD);
    let stored_package = deployment_fixture::cabinet(&case.before, false);
    let compressed_package = deployment_fixture::cabinet(&case.before, true);

    Envelopes {
        case,
        encrypted,
        stored_package,
        compressed_package,
    }
}

#[test]
fn qualify_readable_encrypted_and_deployment_sources_before_index_guard_calls() {
    let inputs = envelopes();
    source::qualify(&inputs.case.before, &inputs.case, false);
    let original = tablecraft_ace::read_table(
        &mut Cursor::new(&inputs.case.before),
        source::scope(),
        "Items",
        &mut source::budget(),
        &|| false,
    )
    .unwrap();

    let header = tablecraft_ace::inspect_header(
        &mut Cursor::new(&inputs.encrypted),
        &mut source::budget(),
        &|| false,
    )
    .unwrap();
    assert!(header.encoded_pages());
    let mut decoded = EncryptedSource::open(
        Cursor::new(&inputs.encrypted),
        Some(PASSWORD),
        &mut source::budget(),
        &|| false,
    )
    .unwrap();
    let actual = tablecraft_ace::read_table(
        &mut decoded,
        source::scope(),
        "Items",
        &mut source::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(actual.rows(), original.rows());
    let checked = tablecraft_ace::check_table(
        &mut decoded,
        source::scope(),
        "Items",
        &mut source::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), inputs.case.rows.len());
    assert_eq!(checked.indexes().len(), 4);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    for (name, expected) in [("PrimaryID", 1000), ("GroupOrder", 2000)] {
        let counter = tablecraft_ace::inspect_table_index_counter(
            &mut decoded,
            source::scope(),
            "Items",
            name,
            &mut source::budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(counter.value(), expected);
    }

    for package in [&inputs.stored_package, &inputs.compressed_package] {
        let mut selected =
            AccessInput::open(Cursor::new(package), &mut source::budget(), &|| false).unwrap();
        assert!(selected.is_deployment());
        let mut payload = Vec::new();
        selected.read_to_end(&mut payload).unwrap();
        source::same_bytes(&payload, &inputs.case.before);
        source::qualify(&payload, &inputs.case, false);
    }
}

#[test]
fn index_guard_refusals_keep_ciphertext_and_deployment_inputs_unchanged() {
    let inputs = envelopes();
    for (bytes, password, expected_error) in [
        (&inputs.encrypted, None, "encrypted rewrite is unsupported"),
        (
            &inputs.encrypted,
            Some(PASSWORD),
            "encrypted rewrite is unsupported",
        ),
        (
            &inputs.stored_package,
            None,
            "deployment packages cannot be engineering index sources",
        ),
        (
            &inputs.compressed_package,
            None,
            "deployment packages cannot be engineering index sources",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("input.accdb");
        let output = directory.path().join("copy.accdb");
        fs::write(&database, bytes).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_tablecraft"));
        command
            .args(["query", "preview-create-index", &source::sql(&inputs.case)])
            .arg("--database")
            .arg(&database)
            .arg("--output")
            .arg(&output)
            .stdin(Stdio::null());
        if let Some(password) = password {
            command.args(["--password", password]);
        }
        let result = command.output().unwrap();

        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(expected_error),
            "{}",
            String::from_utf8_lossy(&result.stderr),
        );
        assert!(!output.exists());
        source::same_bytes(&fs::read(&database).unwrap(), bytes);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
