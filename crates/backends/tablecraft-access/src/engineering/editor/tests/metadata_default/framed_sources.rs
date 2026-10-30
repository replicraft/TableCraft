//! Read-only qualification before switching any frontend mutation fixture.
use super::{cases, framed, preservation, qualify};

#[test]
fn qualify_access_framed_fitting_derivatives_before_frontend_mutation() {
    let originals = cases();
    assert_eq!(originals.len(), 43);
    for original in originals {
        let derived = framed::framed(original);
        qualify(&derived.before, &derived.rows, derived.family);
        qualify(&derived.expected, &derived.final_rows, derived.family);
        preservation(&derived);
    }
}
