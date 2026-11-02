//! Existing preview collection, repair and audit still use the shared kernel.
use super::{Tree, audit_siblings};
use crate::{
    NativeImage,
    format::reader::Reader,
    inspect_table_indexes,
    table::indexes::mutation::{
        NumericKey, encoding, profile::SingleProfile, retirement::RetiredPage,
    },
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::genuine_recursive_retirement as fixtures;

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 10_000_000, 2_000_000_000)
}

#[test]
#[ignore = "requires six qualified recursive retirement SOURCE views; preview feature only"]
fn preview_collect_repair_and_audit_preserve_existing_internal_chain_contract() {
    let cases = fixtures::load_all(|raw, hash| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, hash);
    });
    for case in cases {
        let scope = CatalogScope::new(264);
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            scope,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let inspection = inspect_table_indexes(
            &mut image.reader(),
            scope,
            "BinaryStorage",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let index = &inspection.indexes()[0];
        let profile = SingleProfile {
            column: 0,
            kind: NumericKey::new(ScalarType::Int32).unwrap(),
            descending: false,
        };
        let mut source = image.reader();
        let mut allowance = budget();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget: &mut allowance,
            cancelled: &|| false,
            reads: 0,
        };
        let mut keys = Vec::new();
        let tree = Tree::read::<_, { encoding::MAX_ENTRY_BYTES }>(
            &image,
            &mut reader,
            case.definition as u32,
            index.root_page(),
            &profile,
            0,
            &mut keys,
        )
        .unwrap();
        let mut audit = tree
            .sibling_chains(&image, index, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(audit.pages.len(), 497);
        assert_eq!(audit.levels.len(), 2);
        assert!(!audit.empty_root);
        let retired = case
            .retired
            .iter()
            .map(|&number| RetiredPage {
                number,
                physical: index.data_number(),
                usage: index.usage_map(),
                root: index.root_page(),
            })
            .collect::<Vec<_>>();
        let mut patches = Vec::new();
        audit
            .repair(
                &image,
                false,
                &retired,
                &mut patches,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        for patch in patches {
            let end = patch.offset + patch.before.len();
            assert_eq!(&case.before[patch.offset..end], patch.before);
            assert_eq!(&case.after[patch.offset..end], patch.after);
        }
        audit_siblings(
            &case.before,
            &case.after,
            &[audit],
            &retired,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }
}
