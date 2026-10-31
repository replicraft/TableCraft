//! Source-only fixtures built by the existing catalog/MR2 owner, before binding.
use super::super::guard_sources::model as original;
use super::model as observed;
use crate::{NativeImage, TableExtraCoverage, read_table};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(7);
const SEQUENTIAL: &str = "<Statements><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[Score]+[Old].[Score]</Argument></Action><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[Score]+1</Argument></Action></Statements>";
const FUTURE: &str = "<Statements><Action Name=\"FutureAction\"/></Statements>";

#[derive(Clone, Copy)]
enum Case {
    Sequential,
    RequiredAfter,
    UnsupportedBody,
    NamesOnly,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 64_000_000)
}

fn source(code: u8, case: Case) -> Vec<u8> {
    match case {
        Case::Sequential => original::source(code, "BeforeChange", SEQUENTIAL).0,
        Case::RequiredAfter => original::source(code, "AfterUpdate", "<Statements/>").0,
        Case::UnsupportedBody => original::source(code, "BeforeChange", FUTURE).0,
        Case::NamesOnly => observed::source(code, observed::Case::NamesOnly).bytes,
    }
}

#[test]
fn qualify_selector_free_binding_sources_before_native_dispatch() {
    for code in [5, 6] {
        for case in [
            Case::Sequential,
            Case::RequiredAfter,
            Case::UnsupportedBody,
            Case::NamesOnly,
        ] {
            let bytes = source(code, case);
            let image =
                NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let rows = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(
                rows.rows(),
                original::ORIGINAL_ROWS
                    .iter()
                    .map(|row| row.iter().copied().map(Value::Int32).collect::<Vec<_>>())
                    .collect::<Vec<_>>()
            );
            let extra = image
                .inspect_table_extra(SCOPE.object_id(10), &mut budget(), &|| false)
                .unwrap();
            assert_eq!(extra.coverage(), TableExtraCoverage::WideMr2);
            assert_eq!((extra.catalog_page(), extra.catalog_slot()), (4, 2));
            let inventory = extra
                .inspect_macro_inventory(&mut budget(), &|| false)
                .unwrap();
            if matches!(case, Case::NamesOnly) {
                assert!(inventory.properties().is_empty());
                assert_eq!(inventory.unrepresented_names().len(), 2);
                assert_eq!(extra.names(), ["ValidateLedger", "RecomputeBalance"]);
            } else {
                assert_eq!(extra.names(), ["StoredGuard"]);
                assert_eq!(inventory.properties().len(), 1);
                let crate::MacroPropertyObservation::Envelopes(xml) =
                    inventory.properties()[0].observation()
                else {
                    panic!("literal XML source must inspect");
                };
                assert_eq!(xml.definitions().len(), 1);
                assert_eq!(
                    xml.definitions()[0].event(),
                    Some(if matches!(case, Case::RequiredAfter) {
                        "AfterUpdate"
                    } else {
                        "BeforeChange"
                    })
                );
            }
            assert_eq!(image.as_bytes(), bytes);
        }
    }
}

mod api;
