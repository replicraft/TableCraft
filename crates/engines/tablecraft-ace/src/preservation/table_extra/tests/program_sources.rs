//! Qualify literal property bytes and source-span goldens before program APIs.
use super::super::{
    TableExtraCoverage, decode,
    program_fixtures::{Outcome, sources},
};
use super::{budget, fixture};

#[test]
fn qualify_documented_desktop_macro_program_sources_before_inspection() {
    let mut counts = [0_usize; 4];
    for source in sources() {
        let native_xml = fixture::text(source.xml);
        let raw = fixture::bag(&[
            fixture::names(&["StoredRule"]),
            fixture::group(3, &[fixture::property(0, 1, 12, &native_xml)]),
        ]);
        let original = raw.clone();
        let inspected = decode::decode(&raw, &mut budget(), &|| false).unwrap();
        assert_eq!(inspected.coverage, TableExtraCoverage::WideMr2);
        assert_eq!(inspected.names, ["StoredRule"]);
        assert_eq!(inspected.groups.len(), 1);
        let properties = inspected.groups[0].properties().unwrap();
        assert_eq!(properties.len(), 1);
        assert_eq!(properties[0].name(), "StoredRule");
        assert_eq!(properties[0].native_type(), 12);
        assert_eq!(properties[0].flags(), 1);
        assert_eq!(properties[0].raw(), native_xml);
        assert_eq!(properties[0].text(), Some(source.xml));
        assert_eq!(raw, original);

        let xml = source.xml.as_bytes();
        for (position, node) in source.nodes.iter().enumerate() {
            let (start, end) = node.span;
            let retained = xml.get(start..end).unwrap();
            assert!(retained.starts_with(b"<"), "{} node{position}", source.name);
            assert!(std::str::from_utf8(retained).is_ok());
            assert!(node.definition < source.definitions);
            if let Some(parent) = node.parent {
                assert!(parent < position);
                let parent = &source.nodes[parent];
                assert!(parent.span.0 <= start && end <= parent.span.1);
            }
            for content in [node.content, node.number, node.description]
                .into_iter()
                .flatten()
            {
                assert!(start <= content.0 && content.0 <= content.1 && content.1 <= end);
                assert!(std::str::from_utf8(&xml[content.0..content.1]).is_ok());
            }
            if node.kind == "Opaque" {
                assert!(node.diagnostic.is_some());
            } else {
                assert!(node.diagnostic.is_none());
            }
        }
        let slot = match source.outcome {
            Outcome::RecognizedDesktopSubset => 0,
            Outcome::ContainsOpaqueStructure => 1,
            Outcome::Malformed => 2,
            Outcome::Unsupported => 3,
        };
        counts[slot] += 1;
    }
    assert_eq!(counts, [6, 13, 2, 4]);
}
