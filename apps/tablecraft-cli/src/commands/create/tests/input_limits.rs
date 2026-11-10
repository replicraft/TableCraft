use super::*;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

fn padded_tsv() -> String {
    // One quoted present decimal Int32, independently spelled with leading zeroes.
    // Its serialized size exceeds 64 KiB while the native value remains just 42.
    format!("Id\n\"{}42\"\n", "0".repeat(70_000))
}

fn with_allowance(output: &Path, input: &Path, option: &str, value: &str) -> Arguments {
    let command = Cli::try_parse_from([
        "tablecraft",
        "create",
        output.to_str().unwrap(),
        "--table",
        "Items",
        "--column",
        "Id",
        "--data",
        input.to_str().unwrap(),
        option,
        value,
    ])
    .unwrap()
    .command
    .unwrap();
    match command {
        Command::Create(arguments) => arguments,
        _ => panic!("expected create"),
    }
}

#[test]
fn quoted_tsv_above_64kib_captures_exactly_and_parses_without_a_file_size_ceiling() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("rows.tsv");
    let original = padded_tsv();
    assert!(original.len() > 65_536);
    fs::write(&path, &original).unwrap();
    let mut pool = ResourceBudget::new(16_000_000, 100, 10_000_000);
    let captured = input::capture(&path, &mut pool, &|| false).unwrap();
    assert_eq!(captured, original.as_bytes());
    assert_eq!(
        input::rows(&captured, &["Id".into()], &mut pool, &|| false).unwrap(),
        [vec![42]]
    );
    assert_eq!(fs::read(&path).unwrap(), original.as_bytes());
}

#[test]
fn large_valid_tsv_creates_one_native_value_and_reopens_independently() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("rows.tsv");
    let output = root.path().join("new.accdb");
    let original = padded_tsv();
    fs::write(&input, &original).unwrap();
    let mut report = Vec::new();
    execute(
        arguments(&output, "Items", &["Id".into()], Some(&input)),
        None,
        &mut report,
    )
    .unwrap();
    let mut file = fs::File::open(&output).unwrap();
    let table = tablecraft_ace::read_table(
        &mut file,
        CatalogScope::new(1),
        "Items",
        &mut ResourceBudget::new(16_000_000, 100, 10_000_000),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows(), [vec![Value::Int32(42)]]);
    assert!(String::from_utf8(report).unwrap().contains("Rows\t1\n"));
    assert_eq!(fs::read(&input).unwrap(), original.as_bytes());
}

#[test]
fn explicit_creation_allowances_refuse_large_input_without_publishing_a_database() {
    for (option, value, resource) in [
        ("--work-bytes", "65536", Resource::Bytes),
        ("--work-rows", "0", Resource::Rows),
        ("--work-steps", "100", Resource::Steps),
    ] {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("rows.tsv");
        let output = root.path().join("new.accdb");
        let original = padded_tsv();
        fs::write(&input, &original).unwrap();
        let mut report = Vec::new();
        let error = execute(
            with_allowance(&output, &input, option, value),
            None,
            &mut report,
        )
        .unwrap_err();
        assert!(
            matches!(error, Error::Native(InspectError::Limit(limit)) if limit.resource == resource)
        );
        assert!(report.is_empty());
        assert!(!output.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        assert_eq!(fs::read(&input).unwrap(), original.as_bytes());
    }
}

#[test]
fn large_tsv_capture_and_parsing_still_honor_cancellation() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("rows.tsv");
    let original = padded_tsv();
    fs::write(&path, &original).unwrap();
    let mut pool = ResourceBudget::new(16_000_000, 100, 10_000_000);
    assert!(matches!(
        input::capture(&path, &mut pool, &|| true),
        Err(Error::Native(InspectError::Cancelled))
    ));
    let calls = std::cell::Cell::new(0_u32);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() > 10
    };
    assert!(matches!(
        input::rows(original.as_bytes(), &["Id".into()], &mut pool, &cancelled),
        Err(Error::Native(InspectError::Cancelled))
    ));
    assert_eq!(fs::read(&path).unwrap(), original.as_bytes());
}
