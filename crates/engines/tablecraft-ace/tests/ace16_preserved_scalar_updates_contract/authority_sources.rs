//! Readable ownership/allocation contradictions before preserved writer APIs.
use super::{SCOPE, budget, catalog_flag, model};
use std::io::Cursor;
use tablecraft_ace::{IndexProjection, IndexProjectionSelection, read_table_index_projection};
use tablecraft_core::values::Value;

#[derive(Clone, Copy)]
pub(super) enum Contradiction {
    UnreachableIndexOwnedData,
    FreeOrdinaryData,
    FreeIndexPage,
}

pub(super) fn source(contradiction: Contradiction) -> model::Case {
    let mut case = catalog_flag::source();
    let (container, slot, member) = match contradiction {
        Contradiction::UnreachableIndexOwnedData => (30, 1, 12),
        Contradiction::FreeOrdinaryData => (1, 0, 12),
        Contradiction::FreeIndexPage => (1, 0, 28),
    };
    for bytes in [&mut case.before, &mut case.expected] {
        let base = container * 4096;
        let at = base + 14 + slot * 2;
        let row = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize;
        let bit = base + row + 5 + member / 8;
        assert_eq!(bytes[bit] & (1 << (member % 8)), 0);
        bytes[bit] |= 1 << (member % 8);
    }
    case
}

#[test]
fn qualify_preserved_ownership_contradictions_as_readable_sources_before_writer() {
    for contradiction in [
        Contradiction::UnreachableIndexOwnedData,
        Contradiction::FreeOrdinaryData,
        Contradiction::FreeIndexPage,
    ] {
        let case = source(contradiction);
        assert_eq!(
            case.before
                .iter()
                .zip(&case.expected)
                .enumerate()
                .filter_map(
                    |(at, (before, after))| (before != after).then_some((at, before ^ after))
                )
                .collect::<Vec<_>>(),
            [(model::BOOLEAN_PATCH_OFFSET, 4)]
        );
        // Native selected reads can be healthy while granting no mutation role.
        for bytes in [&case.before, &case.expected] {
            for index in ["NumberKey", "PrimaryKey"] {
                let selected = read_table_index_projection(
                    &mut Cursor::new(bytes),
                    SCOPE,
                    "Items",
                    index,
                    IndexProjection {
                        columns: &["ID"],
                        selection: IndexProjectionSelection::All,
                    },
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    selected.rows(),
                    &[
                        vec![Value::Int32(1)],
                        vec![Value::Int32(2)],
                        vec![Value::Int32(3)]
                    ]
                );
            }
        }
    }
}
