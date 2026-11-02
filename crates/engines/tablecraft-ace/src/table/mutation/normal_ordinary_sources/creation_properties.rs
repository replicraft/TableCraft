//! Read-only declaration/constraint SOURCE checks, before any matching creator.
use crate::{
    ConstraintViolation, InspectError, NativeImage, PropertyGroupKind, RuleCheckStatus,
    check_table, check_table_constraints, inspect_table_properties, read_table,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::FieldConstraintViolation;
use tablecraft_testkit::fixtures::access::canonical_creation_properties::{
    self as sources, Violation,
};

const SCOPE: CatalogScope = CatalogScope::new(218);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn literal_creation_properties_preserve_all_rows_and_distinguish_declared_constraints() {
    let mut healthy = 0;
    let mut refused = 0;
    for case in sources::CASES {
        let source = sources::source(case);
        for header in [5_u32, 6] {
            let mut bytes = source.bytes.clone();
            bytes[20..24].copy_from_slice(&header.to_le_bytes());
            let image =
                NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(table.rows(), source.rows);
            assert_eq!(
                table
                    .schema()
                    .columns()
                    .iter()
                    .map(|field| field.name())
                    .collect::<Vec<_>>(),
                source
                    .columns
                    .iter()
                    .map(|field| field.name)
                    .collect::<Vec<_>>(),
            );
            let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(checked.rows(), source.rows.len());
            let properties = inspect_table_properties(
                &mut image.reader(),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                properties.raw_bytes(),
                source.property_bytes.as_deref().unwrap_or(&[])
            );
            assert_eq!(properties.groups().len(), source.policies.len());
            if source.property_bytes.is_some() {
                crate::properties::framing::validate(&properties, &mut budget(), &|| false)
                    .unwrap();
            }
            for (group, policy) in properties.groups().iter().zip(&source.policies) {
                assert_eq!(group.kind(), PropertyGroupKind::Column);
                assert_eq!(group.name(), policy.column);
                assert_eq!(
                    group.properties().len(),
                    usize::from(policy.required.is_some())
                        + usize::from(policy.allow_zero_length.is_some())
                );
                for (name, declared) in [
                    ("Required", policy.required),
                    ("AllowZeroLength", policy.allow_zero_length),
                ] {
                    if let Some(value) = declared {
                        let actual = group.get(name).unwrap();
                        assert_eq!(actual.native_type(), 1);
                        assert_eq!(actual.flags(), 1);
                        assert_eq!(actual.raw_value(), &[if value { 255 } else { 0 }]);
                        assert_eq!(actual.boolean().unwrap(), value);
                    } else {
                        assert!(group.get(name).is_none());
                    }
                }
            }
            let constraints = check_table_constraints(
                &mut image.reader(),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            );
            if let Some(violation) = source.violation {
                let expected = match violation {
                    Violation::Required => FieldConstraintViolation::Required,
                    Violation::EmptyText => FieldConstraintViolation::EmptyText,
                };
                assert!(matches!(constraints,
                    Err(InspectError::Constraint(ConstraintViolation::Field(actual))) if actual == expected));
                assert!(
                    super::super::normal_ordinary::capture(&image, "Items", &mut budget(), &|| {
                        false
                    },)
                    .is_err()
                );
                refused += 1;
            } else {
                let constraints = constraints.unwrap();
                for field in constraints.fields() {
                    let policy = source
                        .policies
                        .iter()
                        .find(|policy| policy.column == field.name());
                    let required = policy.and_then(|policy| policy.required);
                    let empty = policy.and_then(|policy| policy.allow_zero_length);
                    assert_eq!(
                        field.required(),
                        if required.is_some() {
                            RuleCheckStatus::Checked
                        } else {
                            RuleCheckStatus::NotDeclared
                        }
                    );
                    assert_eq!(
                        field.empty_text(),
                        if empty.is_some() {
                            RuleCheckStatus::Checked
                        } else {
                            RuleCheckStatus::NotDeclared
                        }
                    );
                }
                let captured =
                    super::super::normal_ordinary::capture(&image, "Items", &mut budget(), &|| {
                        false
                    })
                    .unwrap();
                assert_eq!(captured.table.rows(), source.rows);
                healthy += 1;
            }
        }
    }
    assert_eq!((healthy, refused), (14, 4));
}

#[test]
#[ignore = "explicit create-new property SOURCE export; no creator or mutation"]
fn export_literal_creation_property_sources() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_CREATION_PROPERTIES_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let mut manifest = String::new();
    for case in sources::CASES {
        let source = sources::source(case);
        for header in [5_u32, 6] {
            let mut bytes = source.bytes.clone();
            bytes[20..24].copy_from_slice(&header.to_le_bytes());
            let name = format!("h{header}-{}.accdb", case.id());
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join(&name))
                .unwrap();
            output.write_all(&bytes).unwrap();
            let sha = Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            manifest.push_str(&format!("{name}\t{}\t{sha}\n", bytes.len()));
        }
    }
    assert_eq!(manifest.lines().count(), 18);
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("manifest.tsv"))
        .unwrap();
    output.write_all(manifest.as_bytes()).unwrap();
}
