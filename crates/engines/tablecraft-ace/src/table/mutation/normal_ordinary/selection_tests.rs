//! A weak routing observation is deliberately insufficient for capability.
use crate::NativeImage;
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_ordinary_content::{self as model, Case};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(115),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn ordinary_capability_requires_complete_content_for_all_dynamic_user_tables() {
    let mut files = 0;
    let mut tables = 0;
    for header in [5, 6] {
        for case in model::CASES {
            let mut source = model::source(case);
            if !source.ordinary {
                continue;
            }
            source.bytes[20] = header;
            let original = image(&source.bytes);
            for table in &source.tables {
                assert!(
                    original
                        .select_native_ordinary_fixed_table(&table.name, &mut budget(), &|| false)
                        .unwrap(),
                    "{}",
                    case.id()
                );
                original
                    .check_native_ordinary_fixed_table(&table.name, &mut budget(), &|| false)
                    .unwrap_or_else(|error| panic!("{}: {error:?}", case.id()));
                tables += 1;
            }
            assert_eq!(original.as_bytes(), source.bytes);
            files += 1;
        }
    }
    assert_eq!(files, 24);
    assert_eq!(tables, 56);
}

#[test]
fn routing_hint_cannot_turn_unknown_physical_content_into_capability() {
    for header in [5, 6] {
        for case in [
            Case::OrphanZeroPage,
            Case::OrphanPayloadPage,
            Case::UnknownFreeByte,
            Case::MapAlias,
            Case::SharedSystemMap,
            Case::MissingLvalPairs,
            Case::DuplicateLvalMap,
        ] {
            let mut source = model::source(case);
            source.bytes[20] = header;
            let original = image(&source.bytes);
            assert!(
                original
                    .select_native_ordinary_fixed_table("Items", &mut budget(), &|| false)
                    .unwrap(),
                "{} must exercise the insufficient hint",
                case.id()
            );
            assert!(
                original
                    .check_native_ordinary_fixed_table("Items", &mut budget(), &|| false)
                    .is_err(),
                "{} must fail complete proof",
                case.id()
            );
            assert_eq!(original.as_bytes(), source.bytes);
        }
    }
}

#[test]
fn positive_and_unknown_program_carriers_remain_on_the_existing_route() {
    for header in [5, 6] {
        for case in [
            Case::PositiveProgram,
            Case::OpaqueExtra,
            Case::PresentEmptyExtra,
        ] {
            let mut source = model::source(case);
            source.bytes[20] = header;
            let original = image(&source.bytes);
            assert!(
                !original
                    .select_native_ordinary_fixed_table("Items", &mut budget(), &|| false)
                    .unwrap()
            );
            assert!(
                original
                    .check_native_ordinary_fixed_table("Items", &mut budget(), &|| false)
                    .is_err()
            );
            if case == Case::PositiveProgram {
                original
                    .check_native_before_change_int32_table("Items", &mut budget(), &|| false)
                    .unwrap();
            }
            assert_eq!(original.as_bytes(), source.bytes);
        }
    }
}
