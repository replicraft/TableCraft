//! Long same-owner user/catalog chains preserve logical rows and native identities.
use std::{fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::{InspectError, inspect_catalog, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        Case, readdition::Target as Field,
    },
    canonical_retained_add_table::{
        self as added, Target,
        rows::{self, Input, Profile},
    },
    canonical_table_drop::current as dropped,
    raw_forwarding_chain,
};

const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn source() -> (Vec<u8>, usize) {
    let old = dropped::source(
        Case::MixedBinary,
        Field::ShortText,
        true,
        dropped::History::Inserted,
    );
    let retired = dropped::after(&old);
    let catalog_root = word(&retired, 4 * PAGE + 12);
    let source = added::desktop::append(&retired, "Table1", Target::GeneratedText);
    let source = rows::append(&source, 11, Profile::DesktopText, Input::Text("日本語🦀"));
    (
        rows::append(&source, 11, Profile::DesktopText, Input::Null),
        catalog_root,
    )
}

fn expected() -> Vec<Vec<Value>> {
    vec![
        vec![Value::Int32(1), Value::Text("日本語🦀".into())],
        vec![Value::Int32(2), Value::Null],
    ]
}

#[test]
fn long_catalog_and_user_chains_keep_root_identity_and_fresh_query_results() {
    let (before, catalog_root) = source();
    let scope = CatalogScope::new(3321);
    let old_catalog =
        inspect_catalog(&mut Cursor::new(&before), scope, &mut budget(), &|| false).unwrap();
    let user = raw_forwarding_chain::extend(&before, 12, 0, 20);
    let catalog = raw_forwarding_chain::extend(&before, 4, catalog_root, 20);
    let both = raw_forwarding_chain::extend(&user, 4, catalog_root, 20);
    let repeated = raw_forwarding_chain::extend(&both, 12, 0, 3);
    assert_eq!(raw_forwarding_chain::extend(&repeated, 12, 0, 0), repeated);
    for bytes in [user, catalog, both, repeated] {
        assert_eq!(&bytes[7 * PAGE..8 * PAGE], &before[7 * PAGE..8 * PAGE]);
        assert_eq!(&bytes[11 * PAGE..12 * PAGE], &before[11 * PAGE..12 * PAGE]);
        assert_eq!(&bytes[13 * PAGE..], &before[13 * PAGE..]);
        let table = read_table(
            &mut Cursor::new(&bytes),
            scope,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(table.rows(), expected());
        let catalog =
            inspect_catalog(&mut Cursor::new(&bytes), scope, &mut budget(), &|| false).unwrap();
        let origins = |catalog: &tablecraft_ace::CatalogInspection| {
            catalog
                .origins()
                .iter()
                .map(|origin| {
                    (
                        origin.native_id(),
                        origin.parent_id(),
                        origin.type_code(),
                        origin.flags(),
                        origin.page(),
                        origin.slot(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(origins(&catalog), origins(&old_catalog));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("forwarded.accdb");
        fs::write(&path, &bytes).unwrap();
        let source = NativeSource::new(
            fs::File::open(&path).unwrap(),
            CatalogScope::new(3322),
            budget(),
            &|| false,
        )
        .unwrap();
        let query = parse_query("SELECT * FROM Table1 ORDER BY ID", Default::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &[], ExecutionOptions::default())
                .unwrap()
                .rows(),
            expected()
        );
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}

#[test]
fn long_chain_cycles_and_foreign_data_owner_refuse_without_partial_rows() {
    let (before, _) = source();
    let good = raw_forwarding_chain::extend(&before, 12, 0, 20);
    let start = word(&good, 12 * PAGE + 14) & 0x1fff;
    let mut cycle = good.to_vec();
    cycle[12 * PAGE + start..12 * PAGE + start + 4].copy_from_slice(&(12_u32 << 8).to_le_bytes());
    let mut foreign = good;
    foreign[12 * PAGE + 4..12 * PAGE + 8].copy_from_slice(&999_u32.to_le_bytes());
    for bytes in [cycle, foreign] {
        assert!(matches!(
            read_table(
                &mut Cursor::new(&bytes),
                CatalogScope::new(3323),
                "Table1",
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Malformed)
        ));
    }
}
