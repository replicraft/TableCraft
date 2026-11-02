//! Immutable all-zero wide roots require current Source before split modeling.
use crate::{
    IndexCheckStatus, NativeImage, check_table_constraints,
    format::{allocation, reader::Reader},
    inspect_table_index_counter, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(288);

struct Case {
    path: &'static str,
    hash: &'static str,
    money: bool,
    table: &'static str,
    definition: u32,
    roots: [u32; 3],
    data: [u32; 3],
    claims: &'static [(usize, u8, u8, bool)],
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 10_000_000, 2_000_000_000)
}

fn cases() -> Vec<Case> {
    include!("wide_split_original_sources/pins.rs")
}

fn rows(money: bool) -> Vec<Vec<Value>> {
    (1..=400)
        .map(|id| {
            vec![
                Value::Int32(id),
                if money {
                    Value::Currency(Currency::from_scaled(0))
                } else {
                    Value::Int64(0)
                },
                Value::Int32(id * 17),
            ]
        })
        .collect()
}

#[test]
#[ignore = "requires two immutable SDK originals and four selected wide tables"]
fn wider_split_originals_require_current_source_complete_aliases_and_represented_future_cells() {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_WIDE_SPLIT_SOURCE_REPO").unwrap());
    let verified: Vec<_> = cases()
        .into_iter()
        .map(|case| {
            let raw = fs::read(directory.join(case.path)).unwrap();
            let actual: String = Sha256::digest(&raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(actual, case.hash);
            (case, raw)
        })
        .collect();
    for (case, raw) in verified {
        let image =
            NativeImage::capture(&mut Cursor::new(&raw), SCOPE, &mut budget(), &|| false).unwrap();
        let catalog = crate::inspect_table_schema(
            &mut image.reader(),
            SCOPE,
            "MSysObjects",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(catalog.columns().len(), 17);
        let source = super::capture(&image, case.table, &mut budget(), &|| false).unwrap();
        assert_eq!(source.table.schema().definition_page(), case.definition);
        assert_eq!(source.table.schema().declared_rows(), 400);
        assert_eq!(source.table.schema().columns().len(), 3);
        assert_eq!(source.records.len(), 400);
        let expected = rows(case.money);
        assert_eq!(source.table.rows(), expected);
        for (position, field) in source.table.schema().columns().iter().enumerate() {
            assert_eq!(field.number(), position);
            assert_eq!(field.flags(), 3);
            assert_eq!(field.extended_flags(), 0);
            assert!(field.fixed());
            assert_eq!(field.storage_length(), [4, 8, 4][position]);
            assert_eq!(field.field.offset, [0, 4, 12][position]);
            assert_eq!(
                field.native_type(),
                if position == 1 {
                    if case.money { 5 } else { 19 }
                } else {
                    4
                }
            );
        }
        for (ordinal, record) in source.records.iter().enumerate() {
            let group = if ordinal < 194 {
                0
            } else if ordinal < 388 {
                1
            } else {
                2
            };
            let slot = ordinal - [0, 194, 388][group];
            assert_eq!(record.reference, (case.data[group] << 8) | slot as u32);
            assert_eq!(
                record.offset,
                case.data[group] as usize * 4096 + 4096 - 19 * (slot + 1)
            );
            let mut body = Vec::from([3, 0]);
            body.extend_from_slice(&((ordinal + 1) as i32).to_le_bytes());
            body.extend_from_slice(&0_i64.to_le_bytes());
            body.extend_from_slice(&(((ordinal + 1) * 17) as i32).to_le_bytes());
            body.push(7);
            assert_eq!(record.data, body);
        }
        let checked = crate::table::indexes::check_table_retained(
            &mut image.reader(),
            SCOPE,
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), 400);
        assert_eq!(checked.indexes().len(), 4);
        for (name, physical) in [
            ("PrimaryID", 0),
            ("Ascending", 1),
            ("Descending", 2),
            ("Alias", 1),
        ] {
            let selected = checked
                .indexes()
                .iter()
                .find(|item| item.index().name() == name)
                .unwrap();
            assert_eq!(selected.status(), IndexCheckStatus::Checked);
            assert_eq!(selected.index().data_number(), physical as u32);
            assert_eq!(selected.index().root_page(), case.roots[physical]);
            let counter = inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                case.table,
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(counter.value(), if physical == 0 { 400 } else { 1 });
            let read = read_table_index(
                &mut image.reader(),
                SCOPE,
                case.table,
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(read.rows(), expected);
            let mut input = image.reader();
            let mut work = budget();
            let mut reader = Reader {
                source: &mut input,
                header: image.catalog().header(),
                budget: &mut work,
                cancelled: &|| false,
                reads: 0,
            };
            assert_eq!(
                allocation::pages(&mut reader, selected.index().usage_map()).unwrap(),
                [case.roots[physical]]
            );
        }
        for physical in 0..3 {
            let at = case.roots[physical] as usize * 4096;
            assert_eq!(&raw[at..at + 2], &[4, 1]);
            assert_eq!(
                u32::from_le_bytes(raw[at + 4..at + 8].try_into().unwrap()),
                case.definition
            );
            assert_eq!(&raw[at + 8..at + 24], &[0; 16]);
            let prefix = if physical == 0 { 0 } else { 9 };
            assert_eq!(
                u16::from_le_bytes(raw[at + 24..at + 26].try_into().unwrap()),
                prefix
            );
            assert_eq!(raw[at + 26], 0);
            assert_eq!(
                u16::from_le_bytes(raw[at + 2..at + 4].try_into().unwrap()),
                if physical == 0 { 16 } else { 2007 }
            );
        }
        let td = case.definition as usize * 4096;
        let candidate_ref = u32::from_le_bytes(raw[td + 59..td + 63].try_into().unwrap());
        let mut input = image.reader();
        let mut work = budget();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget: &mut work,
            cancelled: &|| false,
            reads: 0,
        };
        assert_eq!(
            allocation::pages(&mut reader, source.table.schema().layout.usage).unwrap(),
            case.data
        );
        assert_eq!(
            allocation::pages(&mut reader, candidate_ref).unwrap(),
            [case.data[2]]
        );
        let candidate = case.data[2] as usize * 4096;
        assert_eq!(
            u16::from_le_bytes(raw[candidate + 2..candidate + 4].try_into().unwrap()),
            3830
        );
        assert_eq!(
            u16::from_le_bytes(raw[candidate + 12..candidate + 14].try_into().unwrap()),
            12
        );
        assert_eq!(
            u16::from_le_bytes(raw[candidate + 36..candidate + 38].try_into().unwrap()),
            3868
        );
        for &(offset, mask, old, set) in case.claims {
            assert_eq!(raw[offset], old);
            assert_eq!(raw[offset] & mask != 0, set);
        }
        let constraints = check_table_constraints(
            &mut image.reader(),
            SCOPE,
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(constraints.rows(), 400);
        assert!(constraints.relationships().is_empty());
        assert_eq!(image.as_bytes(), raw);
    }
}
