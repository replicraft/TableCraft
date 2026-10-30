//! The existing --variable flag selects one real joint DATA-placement preview.
use super::{fs, model, qualify, run};

#[path = "../../../../../crates/engines/tablecraft-ace/src/table/mutation/mixed_fitted_tests/growing/model.rs"]
pub(super) mod source_model;

#[test]
fn qualify_cli_variable_batch_sources_and_complete_expected_before_actions() {
    for code in [5, 6] {
        for family in [
            model::Family::Legacy,
            model::Family::Modern,
            model::Family::Binary,
        ] {
            for descending in [false, true] {
                for destination in [
                    source_model::Destination::OriginalPage,
                    source_model::Destination::OtherOwned,
                    source_model::Destination::EndOfFile,
                ] {
                    let case = source_model::case(code, family, descending, destination);
                    qualify(&case.before, &case.rows, case.family);
                    qualify(&case.expected, &case.final_rows, case.family);
                    let at = 12 * model::PAGE + 14 + 3 * 2;
                    let word = u16::from_le_bytes(case.expected[at..at + 2].try_into().unwrap());
                    assert_ne!(word & 0x4000, 0);
                    let start = 12 * model::PAGE + usize::from(word & 0x1fff);
                    assert_eq!(&case.expected[start..start + 4], &case.target.to_le_bytes());
                }
            }
        }
    }
}

#[test]
fn variable_batch_flag_publishes_all_text_growth_destinations_exactly() {
    for code in [5, 6] {
        for family in [model::Family::Legacy, model::Family::Modern] {
            for descending in [false, true] {
                for destination in [
                    source_model::Destination::OriginalPage,
                    source_model::Destination::OtherOwned,
                    source_model::Destination::EndOfFile,
                ] {
                    let case = source_model::case(code, family, descending, destination);
                    let directory = tempfile::tempdir().unwrap();
                    let input = directory.path().join("before.accdb");
                    let output = directory.path().join("after.accdb");
                    fs::write(&input, &case.before).unwrap();
                    let result = run(
                        "UPDATE Items SET Age=40,Name='aa' WHERE Id=4",
                        &input,
                        &output,
                        &["--variable"],
                    );
                    assert!(
                        result.status.success(),
                        "{}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                    assert_eq!(fs::read(output).unwrap(), case.expected);
                    assert_eq!(fs::read(input).unwrap(), case.before);
                }
            }
        }
    }
}

#[test]
fn default_capacity_and_mixed_index_growth_refusals_never_publish() {
    let case = source_model::case(
        6,
        model::Family::Legacy,
        false,
        source_model::Destination::EndOfFile,
    );
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("before.accdb");
    fs::write(&input, &case.before).unwrap();
    for (ordinal, (sql, flags)) in [
        ("UPDATE Items SET Age=40,Name='aa' WHERE Id=4", &[][..]),
        (
            "UPDATE Items SET Age=40,Name='aa' WHERE Id=4",
            &["--variable", "--index-growth"][..],
        ),
        (
            "UPDATE Items SET Age=40,Name='aa' WHERE FALSE",
            &["--variable", "--index-growth"][..],
        ),
        (
            "UPDATE Items SET Name='a',Other='b' WHERE FALSE",
            &["--variable"][..],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let output = directory.path().join(format!("refused{ordinal}.accdb"));
        let result = run(sql, &input, &output, flags);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
        assert_eq!(fs::read(&input).unwrap(), case.before);
    }
    // Existing single-variable dispatch consistency is exercised by the parent
    // metadata-default test; it is not a second independently authored image.
}
