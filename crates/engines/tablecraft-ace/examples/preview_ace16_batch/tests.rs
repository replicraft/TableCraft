use super::{Args, Kind, Mode, SPEC_FIELDS, parse};
use clap::{Parser, ValueEnum, error::ErrorKind};
use tablecraft_core::values::Value;

fn arguments(flags: &[&str]) -> Args {
    let mut arguments = vec![
        "batch-preview",
        "missing-source.accdb",
        "fresh-output.accdb",
        "--set",
        "Items",
        "Group",
        "0",
        "int32",
        "null",
        "-2147483648",
    ];
    arguments.extend_from_slice(flags);
    Args::try_parse_from(arguments).unwrap()
}

#[test]
fn no_flag_keeps_the_original_fitted_dispatch_and_exact_scalar_protocol() {
    let args = arguments(&[]);
    assert_eq!(args.mode(), Mode::Fitted);
    assert!(!args.index_growth);
    assert!(!args.cascade_updates);
    assert_eq!(args.cells.len(), SPEC_FIELDS);
    let kind = Kind::from_str(&args.cells[3], false).unwrap();
    assert_eq!(parse(kind, &args.cells[4]).unwrap(), Value::Null);
    assert_eq!(parse(kind, &args.cells[5]).unwrap(), Value::Int32(i32::MIN));
}

#[test]
fn each_explicit_flag_selects_its_own_existing_engine_entry_point() {
    assert_eq!(arguments(&["--cascade-updates"]).mode(), Mode::Cascading);
    assert_eq!(arguments(&["--index-growth"]).mode(), Mode::IndexGrowth);
    let fitted = arguments(&[]);
    let growing = arguments(&["--index-growth"]);
    assert_eq!(growing.cells, fitted.cells);
    assert_eq!(growing.source, fitted.source);
    assert_eq!(growing.output, fitted.output);
}

#[test]
fn growth_and_cascades_conflict_before_any_source_or_output_is_opened() {
    for flags in [
        ["--index-growth", "--cascade-updates"],
        ["--cascade-updates", "--index-growth"],
    ] {
        let mut args = vec![
            "batch-preview",
            "missing-source.accdb",
            "fresh-output.accdb",
            "--set",
            "Items",
            "Group",
            "0",
            "int32",
            "null",
            "1",
        ];
        args.extend_from_slice(&flags);
        let error = Args::try_parse_from(args).err().unwrap();
        assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
    }
}

#[test]
fn growth_keeps_complete_set_groups_required() {
    for args in [
        vec!["batch-preview", "missing", "fresh", "--index-growth"],
        vec![
            "batch-preview",
            "missing",
            "fresh",
            "--index-growth",
            "--set",
            "Items",
            "Group",
            "0",
            "int32",
            "null",
        ],
    ] {
        assert!(Args::try_parse_from(args).is_err());
    }
}
