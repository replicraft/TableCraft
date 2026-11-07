use super::{FailedOutput, assert_publication_guarantee};
use crate::{
    Cli,
    commands::{
        Command,
        create::{Arguments, Error, execute, input, schema},
    },
};
use clap::Parser;
use csv::WriterBuilder;
use std::{fs, path::Path};
use tablecraft_access::creation::CreationError;
use tablecraft_ace::CreateDatabaseError;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Value},
};
use tablecraft_testkit::fixtures::access::canonical_native17_creation as native17;
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_creation as fixed, canonical_fixed_float_creation as ieee,
    canonical_variable_creation as variable,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn words(
    path: &Path,
    table: &str,
    declarations: &[(String, String)],
    data: Option<&Path>,
) -> Vec<String> {
    let mut words = vec![
        "tablecraft".to_owned(),
        "create".to_owned(),
        path.to_str().unwrap().to_owned(),
        "--table".to_owned(),
        table.to_owned(),
    ];
    for (name, declaration) in declarations {
        words.extend(["--field".to_owned(), name.clone(), declaration.clone()]);
    }
    if let Some(data) = data {
        words.extend(["--data".to_owned(), data.to_str().unwrap().to_owned()]);
    }
    words
}

fn arguments(words: Vec<String>) -> Arguments {
    match Cli::try_parse_from(words).unwrap().command.unwrap() {
        Command::Create(arguments) => arguments,
        _ => panic!("expected actual create dispatch"),
    }
}

fn declaration(native: u8, width: u16, precision: u8, scale: u8) -> String {
    match native {
        1 => "boolean".into(),
        2 => "byte".into(),
        3 => "int16".into(),
        4 => "int32".into(),
        19 => "int64".into(),
        5 => "currency-scaled".into(),
        6 => "single-bits".into(),
        7 => "double-bits".into(),
        8 => "date-bits".into(),
        15 => "guid".into(),
        10 => format!("text:{}", width / 2),
        9 => format!("binary:{width}"),
        16 => format!("decimal:{precision}:{scale}"),
        _ => panic!("unexpected independent SOURCE family"),
    }
}

fn token(value: &Value) -> String {
    let literal = match value {
        Value::Null => return "null".into(),
        Value::Boolean(value) => value.to_string(),
        Value::Byte(value) => value.to_string(),
        Value::Int16(value) => value.to_string(),
        Value::Int32(value) => value.to_string(),
        Value::Int64(value) => value.to_string(),
        Value::Currency(value) => value.scaled().to_string(),
        Value::Single(value) => format!("{:08x}", value.to_bits()),
        Value::Double(value) => format!("{:016x}", value.to_bits()),
        Value::DateTime(value) => format!("{:016x}", value.to_bits()),
        Value::Guid(value) => uuid::Uuid::from_bytes(value.to_bytes()).to_string(),
        Value::Decimal(value) => format!("{}:{}", value.coefficient(), value.scale()),
        Value::Text(value) => value.clone(),
        Value::Binary(value) => value.iter().map(|byte| format!("{byte:02x}")).collect(),
        _ => panic!("unexpected SOURCE carrier"),
    };
    format!("value:{literal}")
}

fn data(declarations: &[(String, String)], rows: &[Vec<Value>]) -> Vec<u8> {
    let mut writer = WriterBuilder::new()
        .delimiter(b'\t')
        .from_writer(Vec::new());
    writer
        .write_record(declarations.iter().map(|(name, _)| name))
        .unwrap();
    for row in rows {
        writer.write_record(row.iter().map(token)).unwrap();
    }
    writer.into_inner().unwrap()
}

fn check_source(
    table: &str,
    declarations: &[(String, String)],
    rows: &[Vec<Value>],
    expected: &[u8],
) {
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("new.accdb");
    let input = root.path().join("rows.tsv");
    fs::write(&input, data(declarations, rows)).unwrap();
    let mut report = Vec::new();
    execute(
        arguments(words(&output, table, declarations, Some(&input))),
        None,
        &mut report,
    )
    .unwrap();
    assert_eq!(fs::read(&output).unwrap(), expected);
    assert_eq!(
        String::from_utf8(report).unwrap(),
        format!(
            "Database\tCreated ACE 2016\nColumns\t{}\nRows\t{}\nDurable output\tFile and captured-directory barriers completed\n",
            declarations.len(),
            rows.len(),
        )
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
}

#[test]
fn typed_create_matches_all_fifteen_fixed_ieee_and_variable_complete_sources() {
    for mut source in fixed::CASES
        .into_iter()
        .map(fixed::source)
        .chain(ieee::CASES.into_iter().map(ieee::source))
    {
        source.bytes = native17::source(source.bytes);
        let declarations = source
            .columns
            .iter()
            .map(|column| {
                (
                    column.name.to_owned(),
                    declaration(column.native_type, column.width, 0, 0),
                )
            })
            .collect::<Vec<_>>();
        check_source(source.table, &declarations, &source.rows, &source.bytes);
    }
    for case in variable::CASES {
        let mut source = variable::source(case);
        source.bytes = native17::source(source.bytes);
        let declarations = source
            .columns
            .iter()
            .map(|column| {
                (
                    column.name.to_owned(),
                    declaration(
                        column.native_type,
                        column.width,
                        column.precision,
                        column.scale,
                    ),
                )
            })
            .collect::<Vec<_>>();
        check_source(source.table, &declarations, &source.rows, &source.bytes);
    }
}

#[test]
fn typed_schema_and_null_literal_empty_protocol_are_explicit() {
    let fields = ["T", "text:32", "B", "binary:16", "D", "decimal:28:4"].map(str::to_owned);
    let schema = schema::parse(&fields, &mut budget(), &|| false).unwrap();
    let rows = input::typed_rows(
        b"T\tB\tD\nnull\tnull\tnull\nvalue:null\tvalue:\tvalue:0:4\nvalue:value:null\t00ff\tvalue:-10001:4\nvalue:\t\tnull\n",
        &schema, &mut budget(), &|| false,
    ).unwrap();
    assert_eq!(rows[0], [Value::Null, Value::Null, Value::Null]);
    assert_eq!(rows[1][0], Value::Text("null".into()));
    assert_eq!(rows[1][1], Value::Binary(Vec::new()));
    assert_eq!(rows[2][0], Value::Text("value:null".into()));
    assert_eq!(rows[2][1], Value::Binary(vec![0, 0xff]));
    assert_eq!(rows[3][0], Value::Text(String::new()));
    assert_eq!(rows[3][1], Value::Binary(Vec::new()));
    assert_eq!(rows[1][2], Value::Decimal(Decimal::new(0, 4).unwrap()));
    assert_eq!(schema[0].column.max_text_utf16_units(), Some(32));
    assert_eq!(schema[1].column.max_binary_bytes(), Some(16));
    assert_eq!(schema[2].column.decimal_constraints().unwrap().scale(), 4);
}

#[test]
fn typed_declarations_refuse_before_input_and_password_remains_first() {
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("absent-parent/new.accdb");
    let absent = root.path().join("absent.tsv");
    for declaration in [
        "text",
        "binary",
        "decimal",
        "text:0",
        "text:256",
        "binary:0",
        "binary:256",
        "decimal:0:0",
        "decimal:29:4",
        "decimal:3:4",
        "decimal:28:4:0",
        "text:+8",
        "binary: 16",
        "extended-date",
        "null",
        "omitted",
        "unknown",
    ] {
        let fields = [("X".to_owned(), declaration.to_owned())];
        let error = execute(
            arguments(words(&output, "Items", &fields, Some(&absent))),
            None,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(
            error.invalid_input(),
            "declaration {declaration}: {error:?}"
        );
        assert!(matches!(
            error,
            Error::Input(_)
                | Error::Creation(CreationError::Create(CreateDatabaseError::Declaration))
        ));
    }
    let fields = [("X".to_owned(), "unknown".to_owned())];
    assert!(matches!(
        execute(
            arguments(words(&output, "Items", &fields, Some(&absent))),
            Some("secret"),
            &mut Vec::new()
        ),
        Err(Error::Password)
    ));
    let mut both = words(&output, "Items", &[("X".into(), "int32".into())], None);
    both.extend(["--column".into(), "Y".into()]);
    assert!(Cli::try_parse_from(both).is_err());
    let mut incomplete = words(&output, "Items", &[], None);
    incomplete.extend(["--field".into(), "X".into()]);
    assert!(Cli::try_parse_from(incomplete).is_err());
    assert!(Cli::try_parse_from(words(&output, "Items", &[], None)).is_err());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn typed_prepublication_value_shape_and_encoding_refusals_create_nothing() {
    for (declaration, contents) in [
        ("text:8", "X\n🙂🙂🙂🙂a\n"),
        ("text:8", "X\n\u{feff}\u{0100}\n"),
        ("binary:16", "X\n0000000000000000000000000000000000ff\n"),
        ("decimal:28:4", "X\nvalue:0:0\n"),
        ("boolean", "X\nnull\n"),
        ("int32", "X\nvalue:not-a-number\n"),
        ("text:8", "Wrong\nvalue:ok\n"),
        ("text:8", "X\nvalue:one\tvalue:two\n"),
        ("text:8", "X\n\"unfinished"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("new.accdb");
        let input = root.path().join("rows.tsv");
        fs::write(&input, contents).unwrap();
        let fields = [("X".to_owned(), declaration.to_owned())];
        let mut report = Vec::new();
        let error = execute(
            arguments(words(&output, "Items", &fields, Some(&input))),
            None,
            &mut report,
        )
        .unwrap_err();
        assert!(
            error.invalid_input(),
            "declaration {declaration}: {error:?}"
        );
        assert!(report.is_empty());
        assert!(!output.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[test]
fn typed_empty_creation_keeps_publication_facts_through_stdout_failures() {
    let mut source = variable::source(variable::Case::Empty);
    source.bytes = native17::source(source.bytes);
    let fields = source
        .columns
        .iter()
        .map(|column| {
            (
                column.name.to_owned(),
                declaration(
                    column.native_type,
                    column.width,
                    column.precision,
                    column.scale,
                ),
            )
        })
        .collect::<Vec<_>>();
    for fail_write in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("new.accdb");
        let error = execute(
            arguments(words(&output, source.table, &fields, None)),
            None,
            &mut FailedOutput { fail_write },
        )
        .unwrap_err();
        assert_eq!(fs::read(&output).unwrap(), source.bytes);
        assert!(!error.invalid_input());
        match error {
            Error::Output { receipt, cause } => {
                assert_publication_guarantee(&receipt);
                assert_eq!((receipt.row_count(), receipt.column_count()), (0, 5));
                assert_eq!(cause.kind(), std::io::ErrorKind::BrokenPipe);
            }
            _ => panic!("known typed publication must survive failed output"),
        }
    }
}

#[test]
fn typed_resource_limits_and_physical_overflow_do_not_publish() {
    let mut source = variable::source(variable::Case::Unicode);
    source.bytes = native17::source(source.bytes);
    let fields = source
        .columns
        .iter()
        .map(|column| {
            (
                column.name.to_owned(),
                declaration(
                    column.native_type,
                    column.width,
                    column.precision,
                    column.scale,
                ),
            )
        })
        .collect::<Vec<_>>();
    for limit in ["--work-bytes", "--work-rows", "--work-steps"] {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("new.accdb");
        let input = root.path().join("rows.tsv");
        fs::write(&input, data(&fields, &source.rows)).unwrap();
        let mut argv = words(&output, source.table, &fields, Some(&input));
        argv.extend([limit.to_owned(), "0".to_owned()]);
        let error = execute(arguments(argv), None, &mut Vec::new()).unwrap_err();
        assert!(!error.invalid_input(), "resource {limit}: {error:?}");
        assert!(!output.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("overflow.accdb");
    let input = root.path().join("rows.tsv");
    // The independent third Unicode SOURCE row is 66 bytes, plus its 2-byte
    // directory slot. 64 such rows exceed the 4096-byte DATA page.
    let rows = vec![source.rows[2].clone(); 64];
    fs::write(&input, data(&fields, &rows)).unwrap();
    let error = execute(
        arguments(words(&output, source.table, &fields, Some(&input))),
        None,
        &mut Vec::new(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        Error::Creation(CreationError::Create(CreateDatabaseError::PageCapacity))
    ));
    assert!(error.invalid_input());
    assert!(!output.exists());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}
