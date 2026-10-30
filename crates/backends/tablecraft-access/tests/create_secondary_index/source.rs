//! Shared test-owned qualification of the actual pure secondary-index corpus.
use std::{collections::BTreeSet, io::Cursor};
use tablecraft_ace::{IndexCheckStatus, NativeImage, RuleCheckStatus};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::secondary_index as model;

const PAGE: usize = 4096;

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

pub fn scope() -> CatalogScope {
    CatalogScope::new(71)
}

pub fn captured(bytes: &[u8], work: &mut ResourceBudget) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), work, &|| false).unwrap()
}

pub fn sql(case: &model::Case) -> String {
    format!(
        "CREATE INDEX [{}] ON [Items] ([Score] {})",
        case.name,
        if case.descending { "DESC" } else { "ASC" }
    )
}

pub fn same_bytes(actual: &[u8], expected: &[u8]) {
    let differences = actual
        .iter()
        .zip(expected)
        .enumerate()
        .filter_map(|(at, (left, right))| (left != right).then_some((at, *left, *right)))
        .take(12)
        .collect::<Vec<_>>();
    assert_eq!(
        actual.len(),
        expected.len(),
        "first differences: {differences:?}"
    );
    assert!(differences.is_empty(), "first differences: {differences:?}");
}

fn page(bytes: &[u8], number: u32) -> &[u8] {
    &bytes[number as usize * PAGE..(number as usize + 1) * PAGE]
}

// Exact authored inline-map framing only. This is not a generic storage reader
// or a replacement for the independently executed native complete-role gate.
fn members(bytes: &[u8], reference: u32) -> BTreeSet<u32> {
    let storage = page(bytes, reference >> 8);
    assert_eq!(storage[0], 1);
    let slot = (reference & 255) as usize;
    let count = usize::from(u16::from_le_bytes(storage[12..14].try_into().unwrap()));
    assert!(slot < count);
    let offset = |position: usize| {
        usize::from(u16::from_le_bytes(
            storage[14 + 2 * position..16 + 2 * position]
                .try_into()
                .unwrap(),
        ))
    };
    let start = offset(slot);
    assert_eq!(start & 0xe000, 0);
    let end = if slot == 0 { PAGE } else { offset(slot - 1) };
    let body = &storage[start..end];
    assert_eq!(body.len(), 69);
    assert_eq!(body[0], 0);
    let base = u32::from_le_bytes(body[1..5].try_into().unwrap());
    (0..512)
        .filter(|bit| body[5 + bit / 8] & (1 << (bit % 8)) != 0)
        .map(|bit| base + bit as u32)
        .collect()
}

pub fn qualify(bytes: &[u8], case: &model::Case, after: bool) {
    let image = captured(bytes, &mut budget());
    let catalog =
        tablecraft_ace::inspect_catalog(&mut image.reader(), scope(), &mut budget(), &|| false)
            .unwrap();
    let names = catalog
        .catalog()
        .objects()
        .iter()
        .map(|object| object.name().as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        names,
        BTreeSet::from(["MSysObjects", "MSysRelationships", "Items", "Sentinel"])
    );

    let table = tablecraft_ace::read_table(
        &mut image.reader(),
        scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.schema().object_id(), scope().object_id(10));
    assert_eq!(table.schema().definition_page(), 10);
    assert_eq!(
        table
            .columns()
            .iter()
            .map(|column| column.name())
            .collect::<Vec<_>>(),
        ["Id", "Score", "Guard"]
    );
    let expected_rows = case
        .rows
        .iter()
        .map(|row| {
            vec![
                Value::Int32(row.id),
                row.score.map_or(Value::Null, Value::Int32),
                Value::Int32(row.guard),
            ]
        })
        .collect::<Vec<_>>();
    assert_eq!(table.rows(), expected_rows);

    let sentinel = tablecraft_ace::read_table(
        &mut image.reader(),
        scope(),
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(sentinel.schema().definition_page(), 18);
    assert_eq!(sentinel.rows(), [vec![Value::Int32(999)]]);

    let constraints = tablecraft_ace::check_table_constraints(
        &mut image.reader(),
        scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), case.rows.len());
    assert!(!constraints.fields().iter().any(|field| {
        [field.required(), field.empty_text(), field.validation()]
            .contains(&RuleCheckStatus::Unsupported)
    }));
    assert_ne!(constraints.table_validation(), RuleCheckStatus::Unsupported);
    assert!(constraints.relationships().is_empty());

    let checked = tablecraft_ace::check_table(
        &mut image.reader(),
        scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), case.rows.len());
    assert_eq!(checked.indexes().len(), if after { 5 } else { 4 });
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );

    assert_eq!(
        u32::from_le_bytes(page(bytes, 10)[51..55].try_into().unwrap()),
        if after { 3 } else { 2 },
    );
    let physical = checked
        .indexes()
        .iter()
        .map(|index| index.index().data_number())
        .collect::<BTreeSet<_>>();
    assert_eq!(physical, (0..if after { 3 } else { 2 }).collect());

    let data_pages = case
        .rows
        .iter()
        .map(|row| row.reference >> 8)
        .chain([12])
        .collect::<BTreeSet<_>>();
    assert_eq!(members(bytes, 11 << 8), data_pages);
    assert_eq!(members(bytes, (11 << 8) | 1), data_pages);
    let mut all_index_pages = BTreeSet::new();
    for (position, name) in ["PrimaryID", "PrimaryAlias", "GroupOrder", "GroupAlias"]
        .into_iter()
        .enumerate()
    {
        let index = checked.indexes()[position].index();
        let physical = (position / 2) as u32;
        assert_eq!(index.name(), name);
        assert_eq!(index.number(), position as u32);
        assert_eq!(index.data_number(), physical);
        assert_eq!(index.root_page(), 13 + physical);
        assert_eq!(index.usage_map(), (15 << 8) | physical);
        assert_eq!(index.columns().len(), 1);
        assert_eq!(
            index.columns()[0].number(),
            if position < 2 { 0 } else { 2 }
        );
        assert_eq!(
            members(bytes, index.usage_map()),
            BTreeSet::from([13 + physical])
        );
        all_index_pages.insert(13 + physical);
        let counter = tablecraft_ace::inspect_table_index_counter(
            &mut image.reader(),
            scope(),
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(counter.value(), if position < 2 { 1000 } else { 2000 });
    }

    if after {
        let index = checked.indexes()[4].index();
        assert_eq!(index.name(), case.name);
        assert_eq!(
            (index.number(), index.data_number(), index.root_page()),
            (4, 2, 25)
        );
        assert_eq!((index.native_type(), index.flags()), (0, 0x80));
        assert_eq!(index.usage_map(), 24 << 8);
        assert_eq!(index.columns().len(), 1);
        assert_eq!(index.columns()[0].number(), 1);
        assert_eq!(index.columns()[0].descending(), case.descending);
        let new_index_pages = case
            .new_pages
            .iter()
            .copied()
            .filter(|number| *number != 24)
            .collect::<BTreeSet<_>>();
        assert_eq!(members(bytes, index.usage_map()), new_index_pages);
        all_index_pages.extend(new_index_pages);
        let counter = tablecraft_ace::inspect_table_index_counter(
            &mut image.reader(),
            scope(),
            "Items",
            case.name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(counter.value(), case.new_counter);
        let selected = tablecraft_ace::read_table_index(
            &mut image.reader(),
            scope(),
            "Items",
            case.name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let selected_rows = case
            .new_stream
            .iter()
            .map(|wire| {
                let reference = u32::from_be_bytes(wire[wire.len() - 4..].try_into().unwrap());
                let row = case
                    .rows
                    .iter()
                    .find(|row| row.reference == reference)
                    .unwrap();
                vec![
                    Value::Int32(row.id),
                    row.score.map_or(Value::Null, Value::Int32),
                    Value::Int32(row.guard),
                ]
            })
            .collect::<Vec<_>>();
        assert_eq!(selected.rows(), selected_rows);
    }

    // Bind all canonical participating DATA/INDEX roles to represented allocated
    // pages, with independent bodies disjoint from map storage and definitions.
    assert!(data_pages.is_disjoint(&all_index_pages));
    let metadata = BTreeSet::from([0, 1, 2, 3, 7, 8, 10, 11, 15, 18, 19]);
    assert!(metadata.is_disjoint(&data_pages));
    assert!(metadata.is_disjoint(&all_index_pages));
    let free = members(bytes, 1 << 8);
    for number in metadata
        .into_iter()
        .chain(data_pages)
        .chain(all_index_pages)
        .chain([4, 20])
    {
        assert!(!free.contains(&number), "allocated role page {number}");
    }
    if after {
        assert!(!free.contains(&24));
    }
    assert_eq!(image.as_bytes(), bytes);
}

pub fn qualify_all() {
    let mut count = 0;
    for code in [5, 6] {
        for population in model::POPULATIONS {
            for descending in [false, true] {
                let case = model::source(code, population, descending);
                qualify(&case.before, &case, false);
                qualify(&case.expected, &case, true);
                for number in 0..24 {
                    if ![1, 10].contains(&number) {
                        same_bytes(page(&case.before, number), page(&case.expected, number));
                    }
                }
                count += 1;
            }
        }
    }
    assert_eq!(count, 16);
}
