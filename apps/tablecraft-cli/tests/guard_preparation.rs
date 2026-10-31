//! SOURCE gates precede explicit guard service and command checks.
#[path = "guard_preparation/api.rs"]
mod api;
use tablecraft_testkit::access::mutation_fixture as base;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/src/preservation/table_extra/tests/fixture.rs"]
mod fixture;
#[path = "guard_preparation/guard_sources.rs"]
mod guard_sources;
#[path = "guard_preparation/inspect.rs"]
mod inspect;
#[path = "guard_preparation/selector_sources.rs"]
mod selector_sources;

use tablecraft_ace::{
    InspectError, MacroProgramCoverage, MacroProgramNodeKind, MacroProgramProfile,
    TableExtraCoverage,
};
use tablecraft_testkit::access as support;

#[test]
fn qualify_guard_host_ordinary_sources_before_commands() {
    let mut images = 0;
    for case in guard_sources::cases::CASES {
        for code in [5, 6] {
            let (bytes, xml, bag) = guard_sources::model::source(code, case.event, case.body);
            let image = inspect::source(&bytes);
            let extra = inspect::extra(&image).unwrap();
            assert_eq!(extra.coverage(), TableExtraCoverage::WideMr2);
            assert_eq!(extra.raw(), Some(bag.as_slice()));
            assert_eq!(extra.groups()[0].kind(), 3);
            let property = &extra.groups()[0].properties().unwrap()[0];
            assert_eq!(property.name(), "StoredGuard");
            assert_eq!(property.text(), Some(xml.as_str()));
            assert_eq!((property.native_type(), property.flags()), (12, 1));
            let program = property
                .inspect_macro_program(
                    MacroProgramProfile::DocumentedDesktop,
                    &mut inspect::budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(
                program.envelopes().definitions()[0].event(),
                Some(case.event)
            );
            let counts = program
                .nodes()
                .iter()
                .fold([0_usize; 3], |mut counts, node| {
                    match node.kind() {
                        MacroProgramNodeKind::Condition { .. } => counts[0] += 1,
                        MacroProgramNodeKind::RaiseError { .. } => counts[1] += 1,
                        MacroProgramNodeKind::Opaque { .. } => counts[2] += 1,
                        _ => {}
                    }
                    counts
                });
            assert_eq!(
                counts,
                [case.conditions, case.raises, case.opaque],
                "{} / header {code}",
                case.name
            );
            assert!(!case.trace.is_empty());
            assert!(image.as_bytes() == bytes);
            images += 1;
        }
    }
    assert_eq!(images, 40);
}

#[test]
fn qualify_guard_host_selector_sources_before_commands() {
    let mut admitted = 0;
    let mut refused = 0;
    for code in [5, 6] {
        for variant in [
            selector_sources::model::Variant::Valid,
            selector_sources::model::Variant::SecondOpaque,
            selector_sources::model::Variant::SecondMalformed,
            selector_sources::model::Variant::DuplicateKind,
        ] {
            let source = selector_sources::model::source(code, variant);
            let image = inspect::source(&source.bytes);
            let result = inspect::extra(&image);
            if matches!(variant, selector_sources::model::Variant::DuplicateKind) {
                assert_eq!(result.err(), Some(InspectError::Malformed));
                refused += 1;
                continue;
            }
            let extra = result.unwrap();
            assert_eq!(extra.coverage(), TableExtraCoverage::WideMr2);
            assert_eq!(extra.raw(), Some(source.bag.as_slice()));
            let properties = extra.groups()[0].properties().unwrap();
            assert_eq!(properties.len(), 2);
            assert_eq!(
                (properties[0].name(), properties[1].name()),
                ("GuardA", "GuardB")
            );
            assert_eq!(properties[0].text(), Some(source.alpha.as_str()));
            assert_eq!(properties[1].text(), Some(source.beta.as_str()));
            assert_eq!(
                properties[0]
                    .inspect_macro_program(
                        MacroProgramProfile::DocumentedDesktop,
                        &mut inspect::budget(),
                        &|| false
                    )
                    .unwrap()
                    .coverage(),
                MacroProgramCoverage::RecognizedDesktopSubset
            );
            assert!(image.as_bytes() == source.bytes);
            admitted += 1;
        }
    }
    assert_eq!((admitted, refused), (6, 2));
}

#[path = "guard_preparation/before_change_sources.rs"]
mod before_change_sources;
