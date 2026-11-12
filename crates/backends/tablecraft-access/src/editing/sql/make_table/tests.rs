//! Independent raw source images; preparation cannot accept foreign result data.
mod computed;
mod computed_declarations;
mod parameters;
mod selection;
mod wildcards;

use super::*;
use std::io::Cursor;
use tablecraft_core::{limits::Resource, values::Decimal};
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, SourceFailure};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    self as raw, desktop, exact_values, fields, rows,
};

const SCOPE: CatalogScope = CatalogScope::new(19004);

fn budget() -> ResourceBudget {
    ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn syntax(sql: &str) -> Query {
    parse_query(sql, Default::default()).unwrap().into_query()
}

fn prepare(bytes: &[u8], sql: &str) -> Result<PreparedNativeMakeTable, NativeMakeTableError> {
    prepare_native_make_table(
        image(bytes),
        syntax(sql),
        &RefCell::new(budget()),
        &mut budget(),
        &[],
        Default::default(),
        TableSnapshotLimits::default(),
    )
}

fn text_source() -> (Vec<u8>, u32) {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (before.len() / 4096) as u32;
    let mut bytes = desktop::append(&before, "Items", raw::Target::GeneratedText);
    // Independently narrow the actual declared field size to seven UTF-16 units.
    // No data or output-string length is used to manufacture this declaration.
    let width = definition as usize * 4096 + 100 + 23;
    bytes[width..width + 2].copy_from_slice(&14_u16.to_le_bytes());
    (bytes, definition)
}

#[test]
fn native_text_capacity_comes_from_actual_metadata_for_empty_and_populated_results() {
    let (mut bytes, definition) = text_source();
    let empty = prepare(
        &bytes,
        "SELECT i.ID AS CopiedId, i.Name AS Label INTO Archive FROM Items AS i",
    )
    .unwrap();
    assert!(empty.rows().is_empty());
    assert!(empty.definition().is_manual());
    assert!(empty.definition().is_native());
    assert_eq!(empty.definition().primary_position(), None);

    let query = syntax("SELECT i.ID AS CopiedId, i.Name AS Label INTO Archive FROM Items AS i");
    let select = shape::select(&query, false).unwrap();
    let schema = inspect_table_schema(
        &mut Cursor::new(&bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let columns = shape::columns(select, &schema, &mut budget(), &|| false).unwrap();
    assert_eq!(columns[1].max_text_utf16_units(), Some(7));
    assert_eq!(columns[1].name(), "Label");
    for column in columns {
        assert_eq!(column.primary_key_name(), None);
        assert!(!column.incrementing_autonumber());
        assert_eq!(column.required(), None);
        assert_eq!(column.allow_zero_length(), None);
        assert_eq!(column.default_literal(), None);
    }

    for input in [
        rows::Input::Text("A🦀"),
        rows::Input::Null,
        rows::Input::Text(""),
    ] {
        bytes = rows::append(&bytes, definition, rows::Profile::DesktopText, input);
    }
    let prepared = prepare(
        &bytes,
        "SELECT ID AS CopiedId, Name AS Label INTO Archive FROM Items ORDER BY ID",
    )
    .unwrap();
    assert_eq!(prepared.original().as_bytes(), bytes.as_slice());
    assert_eq!(
        prepared.rows(),
        &[
            vec![Value::Int32(1), Value::Text("A🦀".into())],
            vec![Value::Int32(2), Value::Null],
            vec![Value::Int32(3), Value::Text("".into())],
        ]
    );
}

#[test]
fn complete_staging_keeps_more_than_sixty_four_rows_without_generation_or_default_backfill() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (before.len() / 4096) as u32;
    let mut bytes = desktop::append(&before, "Items", raw::Target::GeneratedNumber);
    for id in 1..=70 {
        let input = if id == 70 {
            rows::Input::Null
        } else {
            rows::Input::Number(5)
        };
        bytes = rows::append(&bytes, definition, rows::Profile::DesktopNumber, input);
    }
    let prepared = prepare(
        &bytes,
        "SELECT Score AS Repeated INTO Archive FROM Items ORDER BY ID",
    )
    .unwrap();
    assert_eq!(prepared.rows().len(), 70);
    assert!(
        prepared.rows()[..69]
            .iter()
            .all(|row| row == &[Value::Int32(5)])
    );
    assert_eq!(prepared.rows()[69], [Value::Null]);
    assert_eq!(prepared.original().catalog().catalog().scope(), SCOPE);

    let (original, declaration, selected) = prepared.into_parts();
    assert_eq!(original.as_bytes(), bytes.as_slice());
    assert_eq!(declaration.primary_position(), None);
    assert_eq!(declaration.column_count(), 1);
    assert_eq!(selected.len(), 70);
}

#[test]
fn a_truncated_last_record_after_seventy_valid_rows_returns_no_preparation() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (before.len() / 4096) as u32;
    let mut bytes = desktop::append(&before, "Items", raw::Target::GeneratedNumber);
    for value in 1..=70 {
        bytes = rows::append(
            &bytes,
            definition,
            rows::Profile::DesktopNumber,
            rows::Input::Number(value),
        );
    }
    let sql = "SELECT Score INTO Archive FROM Items";
    assert_eq!(prepare(&bytes, sql).unwrap().rows().len(), 70);
    bytes = rows::append(
        &bytes,
        definition,
        rows::Profile::DesktopNumber,
        rows::Input::Number(71),
    );
    let data = (definition as usize + 1) * 4096;
    assert_eq!(
        u16::from_le_bytes(bytes[data + 12..data + 14].try_into().unwrap()),
        71
    );
    let last_slot = data + 14 + 2 * 70;
    let offset = usize::from(
        u16::from_le_bytes(bytes[last_slot..last_slot + 2].try_into().unwrap()) & 0x1fff,
    );
    let previous_slot = last_slot - 2;
    let previous = usize::from(
        u16::from_le_bytes(bytes[previous_slot..previous_slot + 2].try_into().unwrap()) & 0x1fff,
    );
    let truncated = previous - 1;
    assert_eq!(truncated - offset, 10);
    bytes[last_slot..last_slot + 2].copy_from_slice(&(truncated as u16).to_le_bytes());
    let free = u16::from_le_bytes(bytes[data + 2..data + 4].try_into().unwrap());
    bytes[data + 2..data + 4].copy_from_slice(&(free + 10).to_le_bytes());
    // The final directory-framed record contains only one byte, so decoding its
    // two-byte column-count header reaches EOF. Earlier records remain intact;
    // vacated final-row bytes remain inert and the physical free count is honest.
    assert!(matches!(
        prepare(&bytes, sql),
        Err(NativeMakeTableError::Query(MakeTableError::Query(
            QueryError::SourceFailure(SourceFailure::Truncated)
        )))
    ));
}

#[test]
fn decimal_declared_precision_and_scale_survive_aliasing_without_row_inference() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (before.len() / 4096) as u32;
    let bytes = exact_values::append_table(&before, "ExactValues");
    let bytes = exact_values::append(&bytes, definition, None, Some(90_071_992_547_409_931_234));
    let query =
        syntax("SELECT Amount AS Precise, Token AS Identifier INTO Archive FROM ExactValues");
    let schema = inspect_table_schema(
        &mut Cursor::new(&bytes),
        SCOPE,
        "ExactValues",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let columns = shape::columns(
        shape::select(&query, false).unwrap(),
        &schema,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let decimal = columns[0].decimal_constraints().unwrap();
    assert_eq!(decimal.precision(), 28);
    assert_eq!(decimal.scale(), 4);
    let prepared = prepare(
        &bytes,
        "SELECT Amount AS Precise, Token AS Identifier INTO Archive FROM ExactValues",
    )
    .unwrap();
    assert_eq!(
        prepared.rows(),
        &[vec![
            Value::Decimal(Decimal::new(90_071_992_547_409_931_234, 4).unwrap()),
            Value::Null
        ]]
    );
}

#[test]
fn existing_destination_is_rejected_before_invalid_user_data_can_be_scanned() {
    let (mut bytes, definition) = text_source();
    bytes[(definition as usize + 1) * 4096] = 255;
    assert!(matches!(
        prepare(&bytes, "SELECT Name INTO Items FROM Items"),
        Err(NativeMakeTableError::DestinationExists)
    ));
    assert!(prepare(&bytes, "SELECT Name INTO Archive FROM Items").is_err());
}

#[test]
fn existing_saved_query_is_a_destination_collision_without_query_reconstruction() {
    use tablecraft_testkit::fixtures::access::saved_queries;

    let mut bytes = saved_queries::database_with(&[]);
    // This independently authored catalog/query layout is shared by ACE 16.
    // Its intentionally empty saved definition must not be reconstructed here.
    bytes[20] = 5;
    assert!(matches!(
        prepare(&bytes, "SELECT Id INTO Saved FROM Items"),
        Err(NativeMakeTableError::DestinationExists)
    ));
}

#[test]
fn closed_shapes_and_actual_qualifiers_cannot_route_to_external_or_computed_sources() {
    let (bytes, _) = text_source();
    for sql in [
        "SELECT Name & 'x' AS Label INTO Archive FROM Items",
        "SELECT x.Name INTO Archive FROM Items AS i",
        "SELECT Name INTO Archive IN 'unopened.accdb' FROM Items",
        "SELECT Name INTO Archive FROM Items IN 'unopened.accdb'",
        "SELECT Name INTO Archive FROM Items ORDER BY Len(Name)",
        "SELECT i.Name INTO Archive FROM Items AS i, Table1",
    ] {
        assert!(
            matches!(prepare(&bytes, sql), Err(NativeMakeTableError::Shape)),
            "unexpected admission for a refused shape"
        );
    }
    assert!(
        prepare(
            &bytes,
            "SELECT I.nAmE AS Label INTO Archive FROM Items AS i"
        )
        .is_ok()
    );
}

#[test]
fn cancellation_and_native_query_snapshot_allowances_return_no_partial_preparation() {
    let (mut bytes, definition) = text_source();
    for _ in 0..2 {
        bytes = rows::append(
            &bytes,
            definition,
            rows::Profile::DesktopText,
            rows::Input::Null,
        );
    }
    let sql = "SELECT Name INTO Archive FROM Items";
    let cancelled = || true;
    let options = ExecutionOptions {
        cancelled: Some(&cancelled),
        ..ExecutionOptions::default()
    };
    assert!(matches!(
        prepare_native_make_table(
            image(&bytes),
            syntax(sql),
            &RefCell::new(budget()),
            &mut budget(),
            &[],
            options,
            TableSnapshotLimits::default()
        ),
        Err(NativeMakeTableError::Inspect(InspectError::Cancelled))
    ));
    for (native, resource) in [
        (ResourceBudget::new(0, u64::MAX, u64::MAX), Resource::Bytes),
        (ResourceBudget::new(u64::MAX, u64::MAX, 0), Resource::Steps),
    ] {
        let result = prepare_native_make_table(
            image(&bytes),
            syntax(sql),
            &RefCell::new(native),
            &mut budget(),
            &[],
            Default::default(),
            TableSnapshotLimits::default(),
        );
        assert!(matches!(result,
            Err(NativeMakeTableError::Inspect(InspectError::Limit(limit)))
                if limit.resource == resource
        ));
    }
    for (mut query, resource) in [
        (ResourceBudget::new(0, u64::MAX, u64::MAX), Resource::Bytes),
        (ResourceBudget::new(u64::MAX, 0, u64::MAX), Resource::Rows),
        (ResourceBudget::new(u64::MAX, u64::MAX, 0), Resource::Steps),
    ] {
        let result = prepare_native_make_table(
            image(&bytes),
            syntax(sql),
            &RefCell::new(budget()),
            &mut query,
            &[],
            Default::default(),
            TableSnapshotLimits::default(),
        );
        assert!(matches!(result,
            Err(NativeMakeTableError::Query(MakeTableError::Query(QueryError::Limit(limit))))
                if limit.resource == resource
        ));
    }
    assert!(matches!(
        prepare_native_make_table(
            image(&bytes),
            syntax(sql),
            &RefCell::new(budget()),
            &mut budget(),
            &[],
            Default::default(),
            TableSnapshotLimits::new(1, 1).unwrap()
        ),
        Err(NativeMakeTableError::Query(MakeTableError::Query(
            QueryError::SourceFailure(SourceFailure::StructureLimit)
        )))
    ));
}

#[cfg(feature = "journal")]
#[test]
fn complete_preparation_finishes_one_source_bound_populated_candidate() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (before.len() / 4096) as u32;
    let mut bytes = desktop::append(&before, "Items", raw::Target::GeneratedNumber);
    let literals = (0..70)
        .map(|row| (row % 17 != 0).then_some(row))
        .collect::<Vec<_>>();
    for value in &literals {
        bytes = rows::append(
            &bytes,
            definition,
            rows::Profile::DesktopNumber,
            value.map_or(rows::Input::Null, rows::Input::Number),
        );
    }
    let expected = raw::populated::numbers(&bytes, "Archive", &literals);
    let candidate = prepare(
        &bytes,
        "SELECT Score AS CopiedScore INTO Archive FROM Items ORDER BY ID",
    )
    .unwrap()
    .finish(&mut budget(), &|| false)
    .unwrap();
    assert_eq!(candidate.before_bytes(), bytes.as_slice());
    assert_eq!(candidate.row_count(), 70);
    assert_eq!(candidate.after_bytes().len(), expected.len());
    assert_eq!(
        candidate
            .after_bytes()
            .iter()
            .zip(&expected)
            .position(|(a, b)| a != b),
        None,
        "first differing complete populated image byte"
    );
}

#[test]
fn native_comparison_preparation_uses_actual_field_order_and_never_binary_fallback() {
    let (mut bytes, definition) = text_source();
    for text in ["B", "a"] {
        bytes = rows::append(
            &bytes,
            definition,
            rows::Profile::DesktopText,
            rows::Input::Text(text),
        );
    }
    // Independent stored sort descriptors: General legacy on database and field.
    for (index, mask) in [1, 27, 152, 253].into_iter().enumerate() {
        bytes[110 + index] = [9, 4, 0, 0][index] ^ mask;
    }
    let field_order = definition as usize * 4096 + 100 + 11;
    bytes[field_order..field_order + 4].copy_from_slice(&[9, 4, 0, 0]);
    let sql = "SELECT Name AS Label INTO Archive FROM Items ORDER BY Name";
    let prepared = prepare_native_make_table_with_native_comparison(
        image(&bytes),
        syntax(sql),
        &RefCell::new(budget()),
        &RefCell::new(budget()),
        &mut budget(),
        ExecutionOptions::default(),
        TableSnapshotLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepared.rows(),
        &[vec![Value::Text("a".into())], vec![Value::Text("B".into())],]
    );
    assert!(matches!(
        prepare_native_make_table_with_native_comparison(
            image(&bytes),
            syntax(sql),
            &RefCell::new(budget()),
            &RefCell::new(ResourceBudget::new(0, 0, 0)),
            &mut budget(),
            ExecutionOptions::default(),
            TableSnapshotLimits::default(),
        ),
        Err(NativeMakeTableError::Inspect(InspectError::Limit(_)))
    ));
    bytes[field_order..field_order + 4].copy_from_slice(&[31, 4, 0, 1]);
    assert!(matches!(
        prepare_native_make_table_with_native_comparison(
            image(&bytes),
            syntax(sql),
            &RefCell::new(budget()),
            &RefCell::new(budget()),
            &mut budget(),
            ExecutionOptions::default(),
            TableSnapshotLimits::default(),
        ),
        Err(NativeMakeTableError::Query(MakeTableError::Query(
            QueryError::Evaluation(tablecraft_expressions::EvalError::CollationRequired)
        )))
    ));
}
