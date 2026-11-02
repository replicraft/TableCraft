#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_alternate_owned as sources;
use tablecraft_testkit::support::access::owned_growth as workflow;

const SCOPE: CatalogScope = CatalogScope::new(143);

#[test]
#[ignore = "requires all pinned alternate-owned SOURCE, growth and second-edit images"]
fn genuine_alternate_owned_sessions_retain_lease_through_growth_and_forwarded_fitting_edit() {
    let cases = sources::load_all(workflow::verify);
    let saves = workflow::run(
        cases.iter().map(|case| {
            (
                case.before.as_slice(),
                case.after.as_slice(),
                case.second.as_slice(),
            )
        }),
        [sources::OLD, sources::GROWN, sources::SECOND],
        SCOPE,
    );
    assert_eq!(saves, 8);
}
