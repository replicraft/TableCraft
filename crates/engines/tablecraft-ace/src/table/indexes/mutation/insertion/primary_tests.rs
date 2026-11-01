//! Metadata policy separation over source-qualified genuine primary declarations.
use super::{qualify, qualify_actions};
use crate::{EditError, NativeImage, WriteBlocker, check_table};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(199);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires genuine numeric originals qualified before mutation"]
fn wide_primary_is_normal_action_metadata_only_and_legacy_qualifier_stays_int32() {
    let dir = PathBuf::from(std::env::var_os("TABLECRAFT_INTEGRAL_EXTRA_ORIGINALS_DIR").unwrap());
    assert!(dir.is_absolute());
    for (header, hash) in [
        (
            5_u8,
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
        ),
        (
            6_u8,
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
        ),
    ] {
        let raw = fs::read(dir.join(format!("h{header}-integral-extra-original.accdb"))).unwrap();
        let digest = Sha256::digest(&raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(digest, hash);
        let image =
            NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
        for table in [
            "Int64AllNull",
            "CurrencyAllNull",
            "Int64WidePrimary",
            "CurrencyWidePrimary",
        ] {
            let checked =
                check_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
            let normal = qualify_actions(&checked, 0, &mut budget(), &|| false).unwrap();
            assert_eq!(normal.name(), "PrimaryID");
            assert_eq!(normal.columns().len(), 1);
            assert!(!normal.columns()[0].descending());
            if table.ends_with("WidePrimary") {
                assert_eq!(
                    qualify(&checked, 0, &mut budget(), &|| false)
                        .err()
                        .unwrap(),
                    EditError::Unsupported(WriteBlocker::IndexedColumn)
                );
            } else {
                assert_eq!(
                    qualify(&checked, 0, &mut budget(), &|| false)
                        .unwrap()
                        .name(),
                    normal.name()
                );
            }
        }
    }
}
