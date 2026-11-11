//! Qualify literal old/new images without invoking the index-creation producer.
#[cfg(feature = "ace16-create")]
use crate::table::table_append::Inventory;
use crate::{
    IndexCheckStatus, IndexProjectionSelection, IndexReadRequest, InspectError, NativeImage,
    TableCursor, TableSnapshotLimits, check_table, inspect_table_index_counter,
    inspect_table_indexes, read_table_index_with_request,
};
use std::{collections::BTreeSet, io::Cursor};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::secondary_index::normal::{self as raw, Population};

const SCOPE: CatalogScope = CatalogScope::new(19113);

fn allowance(rows: usize) -> ResourceBudget {
    // Test consumer allowances cover complete SOURCE and all independent walks;
    // they are not format ceilings or the default UI snapshot allowance.
    ResourceBudget::new(
        512 * 1024 * 1024,
        rows.max(1) as u64 * 16 + 100_000,
        rows.max(1) as u64 * 20_000 + 4_000_000,
    )
}

fn physical_page(bytes: &[u8], number: u32) -> &[u8] {
    &bytes[number as usize * 4096..(number as usize + 1) * 4096]
}

fn expected_order(case: &raw::Source) -> Vec<raw::Row> {
    let mut rows = case.rows.to_vec();
    rows.sort_unstable_by(|left, right| {
        let key = if case.descending {
            right.score.cmp(&left.score)
        } else {
            left.score.cmp(&right.score)
        };
        key.then(left.reference.cmp(&right.reference))
    });
    rows
}

fn qualify(case: &raw::Source, bytes: &[u8], after: bool, population: &str) {
    let context = format!(
        "{population} rows={} after={after} descending={} levels={} leaves={}",
        case.rows.len(),
        case.descending,
        case.levels,
        case.leaf_pages
    );
    let mut work = allowance(case.rows.len());
    let image = NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut work, &|| false)
        .unwrap();
    // TEMPORARY DIAGNOSTIC: retain the mandatory ownership assertion below,
    // but identify whether independently selected reads already reject the frame.
    if after && case.leaf_pages > 2 {
        let mut diagnostic = allowance(case.rows.len());
        inspect_table_indexes(
            &mut image.reader(),
            SCOPE,
            case.table,
            &mut diagnostic,
            &|| false,
        )
        .unwrap_or_else(|error| panic!("{context}: diagnostic index metadata: {error:?}"));
        check_table(
            &mut image.reader(),
            SCOPE,
            case.table,
            &mut diagnostic,
            &|| false,
        )
        .unwrap_or_else(|error| panic!("{context}: diagnostic table/index check: {error:?}"));
        read_table_index_with_request(
            &mut image.reader(),
            SCOPE,
            case.table,
            case.index_name,
            IndexReadRequest {
                columns: None,
                selection: IndexProjectionSelection::All,
                maximum_rows: case.rows.len().max(1) as u32,
            },
            &mut diagnostic,
            &|| false,
        )
        .unwrap_or_else(|error| panic!("{context}: diagnostic selected index read: {error:?}"));
    }
    // This is complete represented ownership/global allocation observation;
    // it does not mint index mutation or journal permission.
    #[cfg(feature = "ace16-create")]
    Inventory::capture_represented(&image, &mut work, &|| false)
        .unwrap_or_else(|error| panic!("{context}: represented inventory: {error:?}"));
    let metadata =
        inspect_table_indexes(&mut image.reader(), SCOPE, case.table, &mut work, &|| false)
            .unwrap();
    assert_eq!(metadata.schema().definition_page(), case.target_definition);
    assert_eq!(metadata.schema().declared_rows() as usize, case.rows.len());
    let selected = metadata
        .schema()
        .columns()
        .iter()
        .find(|field| field.name() == case.column)
        .unwrap();
    assert_eq!(selected.native_type(), 4);
    assert_eq!(selected.storage_length(), 4);
    let native_number = selected.number();
    let old_count = usize::from(case.table == "CompanionValues");
    assert_eq!(metadata.indexes().len(), old_count + usize::from(after));
    let limits = TableSnapshotLimits::new(case.rows.len().max(1) as u32, 256).unwrap();
    let mut input = image.reader();
    let snapshot = TableCursor::open(&mut input, SCOPE, case.table, &mut work, &|| false)
        .unwrap()
        .into_snapshot(limits, &mut work, &|| false)
        .unwrap();
    assert_eq!(snapshot.rows().len(), case.rows.len());
    let position = snapshot
        .schema()
        .columns()
        .iter()
        .position(|field| field.number() == native_number)
        .unwrap();
    for (record, original) in snapshot.rows().iter().zip(&case.rows) {
        assert_eq!(
            record[position],
            original.score.map_or(Value::Null, Value::Int32)
        );
    }
    let checked =
        check_table(&mut image.reader(), SCOPE, case.table, &mut work, &|| false).unwrap();
    assert_eq!(checked.rows(), case.rows.len());
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    if !after {
        return;
    }
    let new = metadata.indexes().last().unwrap();
    assert_eq!(new.name(), case.index_name);
    assert_eq!(new.number(), old_count as u32);
    assert_eq!(new.data_number(), old_count as u32);
    assert_eq!(new.native_type(), 0);
    assert_eq!(new.flags(), 128);
    assert_eq!(new.root_page(), case.root);
    assert_eq!(new.usage_map(), case.map_host << 8);
    assert_eq!(new.columns().len(), 1);
    assert_eq!(new.columns()[0].number(), native_number);
    assert_eq!(new.columns()[0].descending(), case.descending);
    let distinct = case
        .rows
        .iter()
        .map(|row| row.score)
        .collect::<BTreeSet<_>>()
        .len();
    let counter = inspect_table_index_counter(
        &mut image.reader(),
        SCOPE,
        case.table,
        case.index_name,
        &mut work,
        &|| false,
    )
    .unwrap();
    assert_eq!(counter.value(), distinct as u32);
    let selected_columns = [case.column];
    let indexed = read_table_index_with_request(
        &mut image.reader(),
        SCOPE,
        case.table,
        case.index_name,
        IndexReadRequest {
            columns: Some(&selected_columns),
            selection: IndexProjectionSelection::All,
            maximum_rows: case.rows.len().max(1) as u32,
        },
        &mut work,
        &|| false,
    )
    .unwrap();
    assert_eq!(indexed.rows().len(), case.rows.len());
    for (record, expected) in indexed.rows().iter().zip(expected_order(case)) {
        assert_eq!(record, &[expected.score.map_or(Value::Null, Value::Int32)]);
    }
}

#[test]
fn independent_sources_and_expected_indexes_qualify_empty_nullable_and_real_tree_depth() {
    for population in [
        Population::Empty,
        Population::NullableDuplicate,
        Population::ThreeLeaves,
        Population::MultiLevel,
    ] {
        for descending in [false, true] {
            let case = raw::source(population, descending);
            let label = match population {
                Population::Empty => "Empty",
                Population::NullableDuplicate => "NullableDuplicate",
                Population::ThreeLeaves => "ThreeLeaves",
                Population::MultiLevel => "MultiLevel",
            };
            qualify(&case, &case.before, false, label);
            qualify(&case, &case.expected, true, label);
            if matches!(population, Population::ThreeLeaves) {
                assert!(case.leaf_pages > 2);
            }
            if matches!(population, Population::MultiLevel) {
                assert!(case.levels >= 3);
            }
        }
    }
}

#[test]
fn ordinary_secondary_creation_preserves_existing_primary_typed_payloads_and_siblings() {
    for descending in [false, true] {
        let case = raw::protected_companion(descending);
        qualify(&case, &case.before, false, "ProtectedCompanion");
        qualify(&case, &case.expected, true, "ProtectedCompanion");
        let mut work = allowance(case.rows.len());
        let result = read_table_index_with_request(
            &mut Cursor::new(&case.expected),
            SCOPE,
            case.table,
            case.index_name,
            IndexReadRequest {
                columns: None,
                selection: IndexProjectionSelection::All,
                maximum_rows: 3,
            },
            &mut work,
            &|| false,
        )
        .unwrap();
        for (record, expected) in result.rows().iter().zip(expected_order(&case)) {
            let original_slot = (expected.reference & 255) as usize;
            let (amount, label) = match original_slot {
                0 => (
                    Value::Currency(Currency::from_scaled(12_345)),
                    Value::Text("日本🦀".into()),
                ),
                1 => (Value::Null, Value::Null),
                2 => (
                    Value::Currency(Currency::from_scaled(-99_000)),
                    Value::Text(String::new()),
                ),
                _ => panic!("unexpected independent ROW reference"),
            };
            assert_eq!(
                record,
                &[
                    Value::Int32(original_slot as i32 + 1),
                    expected.score.map_or(Value::Null, Value::Int32),
                    amount,
                    label,
                ]
            );
        }
        for number in 0..case.before.len() / 4096 {
            if number != 1 && number != case.target_definition as usize {
                assert_eq!(
                    physical_page(&case.before, number as u32),
                    physical_page(&case.expected, number as u32)
                );
            }
        }
        let primary_before = inspect_table_indexes(
            &mut Cursor::new(&case.before),
            SCOPE,
            case.table,
            &mut work,
            &|| false,
        )
        .unwrap();
        let primary_after = inspect_table_indexes(
            &mut Cursor::new(&case.expected),
            SCOPE,
            case.table,
            &mut work,
            &|| false,
        )
        .unwrap();
        let before = &primary_before.indexes()[0];
        let after = &primary_after.indexes()[0];
        assert_eq!(before.name(), after.name());
        assert_eq!(before.root_page(), after.root_page());
        assert_eq!(before.usage_map(), after.usage_map());
        assert_eq!(
            physical_page(&case.before, before.root_page()),
            physical_page(&case.expected, after.root_page())
        );
    }
}

#[test]
fn represented_map_owner_and_full_key_row_fence_corruption_are_not_accepted() {
    let case = raw::source(Population::ThreeLeaves, false);
    qualify(
        &case,
        &case.expected,
        true,
        "ThreeLeaves corruption control",
    );
    let mut missing_member = case.expected.clone();
    let map = case.map_host as usize * 4096;
    let record =
        u16::from_le_bytes(missing_member[map + 14..map + 16].try_into().unwrap()) as usize;
    assert_eq!(missing_member[map + record], 0);
    missing_member[map + record + 5] &= !1;
    let mut wrong_owner = case.expected.clone();
    let first = case.index_pages[0] as usize * 4096;
    wrong_owner[first + 4..first + 8].copy_from_slice(&(case.target_definition + 1).to_le_bytes());
    let mut wrong_fence = case.expected.clone();
    let root = case.root as usize * 4096;
    assert_eq!(wrong_fence[root], 3);
    assert_eq!(&wrong_fence[root + 24..root + 26], &[0, 0]);
    let key_length = if wrong_fence[root + 480] == 0 { 1 } else { 5 };
    let row = root + 480 + key_length;
    let reference = u32::from_be_bytes(wrong_fence[row..row + 4].try_into().unwrap());
    assert!(reference > 0);
    // Keep the complete key and child pointer intact; only the ROW part of the
    // parent's upper fence is now smaller than its child's real maximum.
    wrong_fence[row..row + 4].copy_from_slice(&(reference - 1).to_be_bytes());
    for bytes in [missing_member, wrong_owner, wrong_fence] {
        assert!(matches!(
            check_table(
                &mut Cursor::new(bytes),
                SCOPE,
                case.table,
                &mut allowance(case.rows.len()),
                &|| false,
            ),
            Err(InspectError::Malformed)
        ));
    }
}
