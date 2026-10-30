//! Inert read, membership and exact literal-difference qualification only.
use super::corrected;
use super::model::{self, Case, PAGE, Shape};
use std::io::Cursor;
use tablecraft_ace::{
    IndexCheckStatus, InspectError, NativeImage, RuleCheckStatus, check_table,
    check_table_constraints, inspect_catalog, inspect_header, inspect_table_indexes,
    inspect_table_schema, read_table, read_table_projection,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(2 * 1024 * 1024 * 1024, 100_000, 1_000_000_000)
}

fn member(bytes: &[u8], page: usize, slot: usize, number: usize) {
    let span = model::row_span(bytes, page, slot);
    let row = &bytes[span];
    assert_eq!(row.len(), 69);
    assert_eq!(row[0], 0);
    let base = u32::from_le_bytes(row[1..5].try_into().unwrap()) as usize;
    assert!(number >= base && number - base < 512);
    let relative = number - base;
    assert_ne!(row[5 + relative / 8] & (1 << (relative % 8)), 0);
    assert_eq!(
        row[5..].iter().map(|value| value.count_ones()).sum::<u32>(),
        1
    );
}

fn geometry(bytes: &[u8], case: &Case) {
    assert_eq!(bytes.len(), case.shape.length());
    let header = inspect_header(&mut Cursor::new(bytes), &mut budget(), &|| false).unwrap();
    assert_eq!(header.page_count() as usize, bytes.len() / PAGE);
    let catalog =
        inspect_catalog(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    assert_eq!(catalog.header(), header);

    let global = model::row_span(bytes, 1, 0);
    let global_row = &bytes[global];
    assert_eq!(global_row[0], 1);
    assert_eq!(
        u32::from_le_bytes(global_row[1..5].try_into().unwrap()) as usize,
        case.global_bitmap
    );
    assert_eq!(
        &bytes[case.global_bitmap * PAGE..case.global_bitmap * PAGE + 4],
        &[5, 1, 0, 0]
    );
    assert!(
        bytes[case.global_bitmap * PAGE + 4..(case.global_bitmap + 1) * PAGE]
            .iter()
            .all(|&value| value == 0)
    );
    assert!(bytes.len() / PAGE <= (PAGE - 4) * 8);
    member(bytes, 11, 0, case.data);
    member(bytes, 11, 1, case.data);

    let schema = inspect_table_schema(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.declared_rows(), 3);
    assert_eq!(
        schema.columns().len(),
        if case.shape.preserved() { 16 } else { 2 }
    );
    assert_eq!(&bytes[case.data * PAGE..case.data * PAGE + 2], &[1, 1]);
    assert_eq!(
        &bytes[case.data * PAGE + 4..case.data * PAGE + 8],
        &10_u32.to_le_bytes()
    );
    for slot in 0..3 {
        let row = model::row_span(bytes, case.data, slot);
        assert_eq!(
            i32::from_le_bytes(bytes[row.start + 2..row.start + 6].try_into().unwrap()),
            slot as i32 + 1
        );
    }

    let indexes = inspect_table_indexes(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        indexes.indexes().len(),
        if case.shape.preserved() { 3 } else { 4 }
    );
    for index in indexes.indexes() {
        let physical = index.data_number() as usize;
        assert_eq!(index.root_page() as usize, case.roots[physical]);
        assert_eq!(
            index.usage_map(),
            ((case.index_map as u32) << 8) | physical as u32
        );
        member(bytes, case.index_map, physical, case.roots[physical]);
        let page = &bytes[case.roots[physical] * PAGE..(case.roots[physical] + 1) * PAGE];
        assert_eq!(&page[..2], &[4, 1]);
        assert_eq!(&page[4..8], &10_u32.to_le_bytes());
        for slot in 0..3 {
            let reference =
                u32::from_be_bytes(page[485 + slot * 9..489 + slot * 9].try_into().unwrap());
            assert_eq!(reference, ((case.data as u32) << 8) | slot as u32);
        }
    }
}

fn rows(bytes: &[u8], case: &Case, after: bool) {
    if case.shape.preserved() {
        let table = read_table_projection(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &["ID", "Yes_Field", "No_Field"],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            table.rows(),
            &[
                vec![
                    Value::Int32(1),
                    Value::Boolean(after),
                    Value::Boolean(false)
                ],
                vec![Value::Int32(2), Value::Boolean(false), Value::Boolean(true)],
                vec![
                    Value::Int32(3),
                    Value::Boolean(false),
                    Value::Boolean(false)
                ],
            ]
        );
        // These are intentional source-profile refusals, not passing full-scalar
        // checks. The later capture-only phase uses the actual preserved owner.
        assert_eq!(
            read_table(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported)
        );
    } else {
        let checked = check_table(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), 3);
        assert_eq!(checked.indexes().len(), 4);
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        let constraints = check_table_constraints(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(constraints.rows(), 3);
        assert_eq!(constraints.fields().len(), 2);
        assert_eq!(constraints.table_validation(), RuleCheckStatus::NotDeclared);
        assert!(constraints.relationships().is_empty());
        for field in constraints.fields() {
            assert_eq!(field.required(), RuleCheckStatus::NotDeclared);
            assert_eq!(field.empty_text(), RuleCheckStatus::NotDeclared);
            assert_eq!(field.validation(), RuleCheckStatus::NotDeclared);
        }

        let table = read_table(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            table.rows(),
            &[
                vec![Value::Int32(1), Value::Int32(if after { 11 } else { 10 })],
                vec![Value::Int32(2), Value::Int32(20)],
                vec![Value::Int32(3), Value::Int32(30)],
            ]
        );
    }
}

#[test]
fn qualify_larger_fixed_sources_and_complete_expected_before_capture_policy() {
    let mut pairs = 0;
    for code in [5, 6] {
        for shape in Shape::ALL {
            let case = corrected::case(code, shape);
            for (bytes, after) in [(&case.before, false), (&case.expected, true)] {
                geometry(bytes, &case);
                rows(bytes, &case, after);
                if bytes.len() > model::DEFAULT_BYTES {
                    assert!(matches!(
                        NativeImage::capture(
                            &mut Cursor::new(bytes),
                            SCOPE,
                            &mut budget(),
                            &|| false
                        ),
                        Err(InspectError::StructureLimit)
                    ));
                }
            }
            let differences = case
                .before
                .iter()
                .zip(&case.expected)
                .enumerate()
                .filter_map(|(offset, (a, b))| (a != b).then_some(offset))
                .collect::<Vec<_>>();
            assert_eq!(
                differences,
                corrected::selected_offsets(&case),
                "{code} {shape:?}"
            );
            for page in corrected::abandoned_pages(&case) {
                let span = page * PAGE..(page + 1) * PAGE;
                assert_eq!(&case.before[span.clone()], &case.expected[span]);
            }
            pairs += 1;
        }
        let source = model::over_cap_source(code);
        assert_eq!(source.len(), model::EXPLICIT_BYTES + PAGE);
        assert_eq!(
            inspect_header(&mut Cursor::new(&source), &mut budget(), &|| false)
                .unwrap()
                .page_count() as usize,
            source.len() / PAGE
        );
        assert!(matches!(
            NativeImage::capture(&mut Cursor::new(&source), SCOPE, &mut budget(), &|| false),
            Err(InspectError::StructureLimit)
        ));
    }
    assert_eq!(pairs, 12);
}
