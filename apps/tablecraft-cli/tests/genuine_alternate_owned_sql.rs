#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::path::Path;
use tablecraft_testkit::fixtures::access::genuine_alternate_owned as sources;
use tablecraft_testkit::support::cli::owned_growth as workflow;

#[test]
#[ignore = "requires all pinned alternate-owned SOURCE, growth and second-edit images"]
fn genuine_alternate_owned_cli_saves_first_growth_then_fits_same_forwarded_root() {
    let cases = sources::load_all(workflow::verify);
    let saves = workflow::run(
        Path::new(env!("CARGO_BIN_EXE_tablecraft")),
        cases.iter().map(|case| {
            (
                case.before.as_slice(),
                case.after.as_slice(),
                case.second.as_slice(),
            )
        }),
        [sources::GROWN, sources::SECOND],
    );
    assert_eq!(saves, 4);
}
