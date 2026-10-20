use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{
    EditError, InspectError, NativeImage, ReadProfile, WriteBlocker, check_table_constraints,
    inspect_catalog, inspect_table_schema, read_table, read_table_index,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn image(code: u8) -> Vec<u8> {
    // Retagged shared-layout regression input, not an ACE 16-authored fixture.
    // The original generated-family comparisons live in compare_ace16.py.
    let mut bytes = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    bytes[20] = code;
    bytes
}

#[test]
fn ace16_family_profiles_share_bounded_scalar_catalog_and_index_algorithms() {
    for (code, profile) in [(5, ReadProfile::Ace2016), (6, ReadProfile::Ace2019)] {
        let bytes = image(code);
        let mut source = Cursor::new(&bytes);
        let scope = CatalogScope::new(1);
        let catalog = inspect_catalog(&mut source, scope, &mut budget(), &|| false).unwrap();
        assert_eq!(catalog.header().read_profile().unwrap(), profile);
        assert_eq!(catalog.catalog().objects().len(), 146);
        let table =
            read_table(&mut source, scope, "ROULETTE_TB", &mut budget(), &|| false).unwrap();
        assert_eq!(table.rows().len(), 38);
        assert_eq!(table.columns().len(), 3);
        let indexed = read_table_index(
            &mut source,
            scope,
            "ROULETTE_TB",
            "ORDR_NBR",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(indexed.rows().len(), table.rows().len());
        for row in indexed.rows() {
            assert!(table.rows().contains(row));
        }
        let report =
            check_table_constraints(&mut source, scope, "ROULETTE_TB", &mut budget(), &|| false)
                .unwrap();
        assert_eq!(report.rows(), 38);
        assert_eq!(source.get_ref(), &&bytes[..]);
    }
}

#[test]
fn newer_header_read_support_never_grants_native_writes_or_codecs() {
    for code in [5, 6] {
        let mut bytes = image(code);
        let native = NativeImage::capture(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            native
                .plan_int32_updates("ROULETTE_TB", "ORDR_NBR", &[], &mut budget(), &|| false)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::Format))
        );
        bytes[21] = 1;
        let schema = inspect_table_schema(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(schema.declared_rows(), 38);
        let native = NativeImage::capture(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            native.plan_int32_updates("ROULETTE_TB", "ORDR_NBR", &[], &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Format))
        ));
        bytes[21] = 2;
        assert_eq!(
            inspect_table_schema(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "ROULETTE_TB",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported)
        );
        bytes[21] = 0;
        bytes[62] ^= 1;
        assert_eq!(
            inspect_catalog(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Encoded)
        );
    }
}

#[test]
fn ace16_reads_keep_cancellation_budgets_and_malformed_page_failures() {
    for code in [5, 6] {
        let mut bytes = image(code);
        assert_eq!(
            read_table(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "ROULETTE_TB",
                &mut budget(),
                &|| true
            )
            .err(),
            Some(InspectError::Cancelled)
        );
        assert!(matches!(
            read_table(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "ROULETTE_TB",
                &mut ResourceBudget::new(66, 1, 100),
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
        bytes[726 * 4096] = 0;
        assert_eq!(
            inspect_table_schema(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "ROULETTE_TB",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Malformed)
        );
    }
}
