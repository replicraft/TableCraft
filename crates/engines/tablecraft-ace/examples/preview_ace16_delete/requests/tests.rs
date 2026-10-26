use super::*;
use clap::Parser;

fn budget() -> ResourceBudget {
    ResourceBudget::new(8 * 1024 * 1024, 10_000, 1_000_000)
}

#[derive(Parser)]
struct Command {
    #[command(flatten)]
    requests: Requests,
}

#[test]
fn file_and_inline_sources_are_exclusive_and_a_source_is_required() {
    assert!(Command::try_parse_from(["delete"]).is_err());
    assert!(
        Command::try_parse_from(["delete", "--delete", "0:0200", "--delete-file", "input.tsv"])
            .is_err()
    );
    let args =
        Command::try_parse_from(["delete", "--delete", "0:0200", "--delete", "1:0300"]).unwrap();
    let requests = args.requests.load(&mut budget()).unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].expected_bytes(), [2, 0]);
    assert_eq!(requests[1].row(), 1);
    assert!(Command::try_parse_from(["delete", "--delete-file", "input.tsv"]).is_ok());
}

#[test]
fn csv_tsv_quotes_and_hex_case_preserve_exact_ordinals_and_payloads() {
    let input = b"\"0\"\t\"0200010000000A00000003\"\n1\t0200020000001400000003\n";
    let requests = parse_tsv(input, &mut budget()).unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].row(), 0);
    assert_eq!(
        requests[0].expected_bytes(),
        [2, 0, 1, 0, 0, 0, 10, 0, 0, 0, 3]
    );
    assert_eq!(requests[1].row(), 1);
    assert_eq!(
        requests[1].expected_bytes(),
        [2, 0, 2, 0, 0, 0, 20, 0, 0, 0, 3]
    );
}

#[test]
fn invalid_records_fail_without_returning_a_partial_batch() {
    for input in [
        &b""[..],
        &b"\n\n"[..],
        &b"row\thex\n"[..],
        &b"0\n"[..],
        &b"0\t0200\textra\n"[..],
        &b"0\t0200\n1\todd\n"[..],
        &b"0\t0200\n1\tzz\n"[..],
        &b"-1\t0200\n"[..],
    ] {
        assert!(parse_tsv(input, &mut budget()).is_err());
    }
}

#[test]
fn row_payload_and_operation_limits_are_independent() {
    let too_wide = format!("0\t{}\n", "00".repeat(4097));
    assert!(parse_tsv(too_wide.as_bytes(), &mut budget()).is_err());
    let too_many = "0\t0200\n".repeat(4097);
    assert!(parse_tsv(too_many.as_bytes(), &mut budget()).is_err());
    let input = b"0\t0200\n";
    for mut limited in [
        ResourceBudget::new(0, 100, 1_000_000),
        ResourceBudget::new(8 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(8 * 1024 * 1024, 100, 0),
    ] {
        assert!(parse_tsv(input, &mut limited).is_err());
    }
    assert!(parse_tsv(input, &mut budget()).is_ok());
}
