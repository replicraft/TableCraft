//! Explicit opt-in dispatch through the real bounded request/publication path.
use super::*;
use tablecraft_ace::{EditError, WriteBlocker, read_table};

const PHYSICAL_INDEX_BYTES: usize = 52;
const PHYSICAL_FLAGS_OFFSET: usize = 46;
const PRIMARY_REQUIRED_UNIQUE: u8 = 0x89;
const PRIMARY_INDEX_KIND: u8 = 1;
const FOREIGN_INDEX_KIND: u8 = 2;
const CASCADE_ACTION: u8 = 1;
const CASCADE_RELATIONSHIP_FLAG: u32 = 0x1000;

fn cascade_model() -> model::Model {
    let mut value = model::database(6, false, false, false);
    let parent = model::definition(1) * model::PAGE_BYTES;
    let physical = parent + value.logical_start - PHYSICAL_INDEX_BYTES;
    value.bytes[physical + PHYSICAL_FLAGS_OFFSET] = PRIMARY_REQUIRED_UNIQUE;
    value.bytes[parent + value.logical_start + model::INDEX_KIND] = PRIMARY_INDEX_KIND;

    for table in [0, 1] {
        let start = model::definition(table) * model::PAGE_BYTES + value.logical_start;
        for logical in 0..2 {
            let offset = start + logical * model::LOGICAL_BYTES;
            if value.bytes[offset + model::INDEX_KIND] == FOREIGN_INDEX_KIND {
                value.bytes[offset + model::DELETE_ACTION] = CASCADE_ACTION;
            }
        }
    }
    model::relationships(&mut value, CASCADE_RELATIONSHIP_FLAG);
    value
}

#[test]
fn clap_cascade_is_explicit_and_does_not_replace_complete_group_pairs() {
    let ordinary = Args::try_parse_from([
        "batch",
        "source",
        "output",
        "--group",
        "Parents",
        "seeds.tsv",
    ])
    .unwrap();
    assert!(!ordinary.cascade);

    let cascading = Args::try_parse_from([
        "batch",
        "source",
        "output",
        "--cascade",
        "--group",
        "Parents",
        "seeds.tsv",
    ])
    .unwrap();
    assert!(cascading.cascade);
    assert_eq!(
        cascading.groups,
        ["Parents", "seeds.tsv"].map(OsString::from)
    );

    for invalid in [
        vec!["batch", "source", "output", "--cascade"],
        vec![
            "batch",
            "source",
            "output",
            "--cascade",
            "--group",
            "Parents",
        ],
        vec![
            "batch",
            "source",
            "output",
            "--cascade",
            "--delete",
            "0:0200",
        ],
    ] {
        assert!(Args::try_parse_from(invalid).is_err());
    }
}

#[test]
fn cascade_flag_expands_parent_seeds_while_default_refuses_and_output_stays_fresh() {
    let directory = Directory::new();
    let value = cascade_model();
    let source = directory.file("cascade-before.accdb", &value.bytes);
    let requests = model::requests(&value, 1, &[0]);
    let request_file = directory.file("parent.tsv", &tsv(&requests));
    let groups = [("Parents", request_file.as_path())];
    let output = directory.0.join("cascade-after.accdb");

    let error = run(arguments(&source, &output, &groups), &mut budget())
        .err()
        .unwrap();
    assert_eq!(
        error.downcast_ref::<EditError>(),
        Some(&EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    );
    assert!(!output.exists());

    let mut args = arguments(&source, &output, &groups);
    args.cascade = true;
    assert_eq!(run(args, &mut budget()).unwrap(), 3);
    assert_eq!(fs::read(&source).unwrap(), value.bytes);
    let candidate = fs::read(&output).unwrap();
    for (table, expected) in [
        ("Parents", &value.rows[1][1..]),
        ("Items", &value.rows[0][2..]),
    ] {
        let actual = read_table(
            &mut std::io::Cursor::new(&candidate),
            CatalogScope::new(1),
            table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(actual.rows(), expected);
    }

    let mut args = arguments(&source, &output, &groups);
    args.cascade = true;
    let error = run(args, &mut budget()).err().unwrap();
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&output).unwrap(), candidate);
    assert_eq!(fs::read(&source).unwrap(), value.bytes);
}

#[test]
fn cascade_still_captures_malformed_later_file_before_opening_database_or_output() {
    let directory = Directory::new();
    let first = directory.file("first.tsv", b"0\t0200\n");
    let malformed = directory.file("malformed.tsv", b"0\t0200\n1\tnot-hex\n");
    let source = directory.0.join("absent.accdb");
    let output = directory.0.join("absent-output.accdb");
    let groups = [("Parents", first.as_path()), ("Items", malformed.as_path())];
    let mut args = arguments(&source, &output, &groups);
    args.cascade = true;
    let error = run(args, &mut budget()).err().unwrap();
    assert!(
        error
            .to_string()
            .contains("Invalid delete-request record 2")
    );
    assert!(!source.exists());
    assert!(!output.exists());
}
