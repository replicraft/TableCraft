//! Read-only closure/capability checks run before any mutation consumer.
use super::capture;
use crate::{NativeImage, read_table};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_ordinary_indexed as sources;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn counter_index_sources_close_eighteen_originals_and_refuse_thirty_controls() {
    let scope = CatalogScope::new(118);
    let mut accepted = 0;
    let mut refused = 0;
    for header in [5, 6] {
        for identity in sources::CASES {
            let mut source = sources::source(identity);
            source.bytes[20] = header;
            let image = NativeImage::capture(
                &mut Cursor::new(&source.bytes),
                scope,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let result = capture(&image, &mut budget(), &|| false);
            if !source.ordinary {
                assert!(result.is_err(), "{}", identity.id());
                refused += 1;
                continue;
            }
            let certificate = result.unwrap_or_else(|error| panic!("{}: {error:?}", identity.id()));
            let table = read_table(
                &mut image.reader(),
                scope,
                source.table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(table.rows(), source.rows);
            certificate
                .require_source(&image, table.schema(), &mut budget(), &|| false)
                .unwrap();
            // This repeats complete source validation. Its empty actual target
            // must skip a generated/indexed Id rather than hide Score or Flag.
            image
                .check_native_ordinary_fixed_table(source.table, &mut budget(), &|| false)
                .unwrap_or_else(|error| panic!("{}: {error:?}", identity.id()));
            assert_eq!(image.as_bytes(), source.bytes);
            accepted += 1;
        }
    }
    assert_eq!(accepted, 18);
    assert_eq!(refused, 30);
}
