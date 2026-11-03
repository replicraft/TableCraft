//! Actual private NoRequired helpers locate the first admission failure.
use super::{catalog, declarations, properties};
use crate::{EditError, NativeImage, inspect_table_schema, table::mutation::ordinary_content};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(262);

fn stage<T>(header: u8, table: &str, name: &str, result: Result<T, EditError>) -> Option<T> {
    match result {
        Ok(value) => {
            eprintln!("IGNORE_NULL_ADMISSION header={header} table={table} stage={name} ADMITTED");
            Some(value)
        }
        Err(error) => {
            eprintln!(
                "IGNORE_NULL_ADMISSION header={header} table={table} stage={name} REFUSED {error:?}"
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
        "IGNORE_NULL_HEADER format={:?} subversion={} encoded={}",
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
    // These held declarations are all native Int32. Therefore the actual
    // capture_updates dispatcher selects its unchanged DenseIntegral branch.
    assert_eq!(schema.columns().len(), 4);
    assert!(
        schema
            .columns()
            .iter()
            .all(|field| field.native_type() == 4)
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
        eprintln!("IGNORE_NULL_ADMISSION header={header} table={table} properties=RAW_ABSENT");
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
    eprintln!("IGNORE_NULL_ADMISSION header={header} table={table} PRIOR_STAGE_HELPERS_COMPLETE");
}

#[test]
#[ignore = "diagnostic only: existing genuine IgnoreNULL originals, no planner"]
fn observe_actual_no_required_prior_stages_for_existing_ignore_null_originals() {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_IGNORE_NULL_ORIGINAL_DIR").unwrap());
    assert!(directory.is_absolute());
    let images: Vec<_> = [
        (
            5,
            "V2016-source.accdb",
            532480,
            "74f6ac1c99b0244a69427afb379112c7321690b9ba628417b1d9411bbb232258",
        ),
        (
            6,
            "V2019-source.accdb",
            602112,
            "86e518e9d7c9d2b77d72c0457d369748924045c05103aaf6be472f7eecd8bd1a",
        ),
    ]
    .into_iter()
    .map(|(header, file, length, hash)| {
        let raw = fs::read(directory.join(file)).unwrap();
        assert_eq!(raw.len(), length);
        let actual: String = Sha256::digest(&raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
        (header, raw)
    })
    .collect();

    for (header, raw) in images {
        let mut work = ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000);
        let image =
            NativeImage::capture(&mut Cursor::new(&raw), SCOPE, &mut work, &|| false).unwrap();
        for table in ["Single", "UniqueSingle", "RequiredSingle"] {
            observe(&image, header, table);
        }
        assert_eq!(image.as_bytes(), raw);
    }
}
