//! Actual private NoRequired helpers locate the first admission failure.
use super::{catalog, declarations, properties};
use crate::{EditError, NativeImage, inspect_table_schema, table::mutation::ordinary_content};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(296);

fn stage<T>(header: u8, table: &str, name: &str, result: Result<T, EditError>) -> Option<T> {
    match result {
        Ok(value) => {
            eprintln!("GUID_ADMISSION header={header} table={table} stage={name} ADMITTED");
            Some(value)
        }
        Err(error) => {
            eprintln!(
                "GUID_ADMISSION header={header} table={table} stage={name} REFUSED {error:?}"
            );
            None
        }
    }
}

fn observe(image: &NativeImage, header: u8, table: &str) {
    let mut work = ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000);
    let Some(()) = stage(
        header,
        table,
        "standard_capture",
        image.require_standard_capture().map_err(EditError::from),
    ) else {
        return;
    };
    let actual_header = image.catalog().header();
    eprintln!(
        "GUID_HEADER format={:?} subversion={} encoded={}",
        actual_header.format(),
        actual_header.subversion(),
        actual_header.encoded_pages()
    );
    let Some(schema) = stage(
        header,
        table,
        "schema",
        inspect_table_schema(&mut image.reader(), SCOPE, table, &mut work, &|| false)
            .map_err(EditError::from),
    ) else {
        return;
    };
    // ID/Group/Score contain no SHORT_TEXT or SHORT_BINARY. The actual
    // capture_updates dispatcher therefore selects DenseIntegral, before
    // reading any declaration, and never retries DenseRetained on failure.
    assert_eq!(schema.columns().len(), 3);
    assert_eq!(
        schema
            .columns()
            .iter()
            .map(|field| field.native_type())
            .collect::<Vec<_>>(),
        [4, 15, 4],
    );

    let Some(selected) = stage(
        header,
        table,
        "catalog",
        catalog::validate(image, &schema, &mut work, &|| false),
    ) else {
        return;
    };
    if selected.properties_present {
        if stage(
            header,
            table,
            "present_properties",
            properties::capture(image, &schema, &mut work, &|| false),
        )
        .is_none()
        {
            return;
        }
    } else {
        eprintln!("GUID_ADMISSION header={header} table={table} properties=RAW_ABSENT");
    }

    if stage(
        header,
        table,
        "catalog_lval_declarations",
        declarations::capture(image, &mut work, &|| false),
    )
    .is_none()
    {
        return;
    }
    if stage(
        header,
        table,
        "dense_integral_definition",
        ordinary_content::definition::validate_user(image, &schema, &mut work, &|| false),
    )
    .is_none()
    {
        return;
    }

    // Reaching this point is not the complete owner's final macro-coverage
    // check and constructs no NoRequired certificate or mutation authority.
    eprintln!("GUID_ADMISSION header={header} table={table} PRIOR_STAGE_HELPERS_COMPLETE");
}

#[test]
#[ignore = "conditional GUID admission diagnostic: four unchanged indexed originals only"]
fn observe_actual_no_required_prior_stages_for_existing_guid_originals() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_GUID_UPDATE_INDEX_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let images: Vec<_> = [
        (
            5_u8,
            "V2016-Guid-A-pressure-ordinary-source.accdb",
            "4991ffbc914b22d3ae0769e10a7f4c497e8e3066ce8431735a294e620f2a2353",
        ),
        (
            5,
            "V2016-Guid-D-pressure-ordinary-source.accdb",
            "04abb49839ea396628571f6e47fb563311db061c082634215a8e175dab82f036",
        ),
        (
            6,
            "V2019-Guid-A-pressure-ordinary-source.accdb",
            "3ccecfaa6865cecdc932bf239dad0bdfeb770361ef90e0d0d149f91979b1837e",
        ),
        (
            6,
            "V2019-Guid-D-pressure-ordinary-source.accdb",
            "2cd3572d7ec08f5d538a4733d06edc4d8400838a76765bb0a828b343e4dc681d",
        ),
    ]
    .into_iter()
    .map(|(header, file, hash)| {
        let raw = fs::read(directory.join(file)).unwrap();
        let actual = Sha256::digest(&raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, hash);
        assert_eq!(&raw[20..24], &u32::from(header).to_le_bytes());
        (header, file, raw)
    })
    .collect();

    for (header, file, raw) in &images {
        let mut work = ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000);
        let image =
            NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut work, &|| false).unwrap();
        eprintln!("GUID_ADMISSION_ORIGINAL header={header} file={file}");
        observe(&image, *header, "Items");
        assert_eq!(image.as_bytes(), raw);
    }
    assert_eq!(images.len(), 4);
}
