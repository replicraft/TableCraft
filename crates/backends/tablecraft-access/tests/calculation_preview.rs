#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/calculated_storage/fixture.rs"]
mod calculated_fixture;
#[path = "calculation_preview/dependencies.rs"]
mod dependencies;
use tablecraft_testkit::access::mutation_fixture as fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/calculated_memos/fixture.rs"]
mod memo_fixture;
#[path = "../../../engines/tablecraft-ace/tests/calculated_storage/scalar_fixture.rs"]
mod scalar_fixture;
use tablecraft_testkit::access as support;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::{CalculationError, NativeSource};
use tablecraft_ace::{
    InspectError, NativeReadSource, read_stored_calculation, read_stored_int32_calculation,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, Source};
use tablecraft_schema::ScalarType;
use tablecraft_sql::ParseLimits;

const PAGE_BYTES: usize = 4096;
const PARENT_DATA_PAGE: usize = 12;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000)
}

struct Watching {
    data: Cursor<Vec<u8>>,
    forbid_rows: Rc<Cell<bool>>,
}

impl NativeReadSource for Watching {}

impl Read for Watching {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.data.position() as usize / PAGE_BYTES == PARENT_DATA_PAGE {
            assert!(!self.forbid_rows.get(), "preparation read user records");
        }
        self.data.read(bytes)
    }
}

impl Seek for Watching {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.data.seek(position)
    }
}

#[test]
fn preparation_is_metadata_only_and_preview_does_not_replace_or_require_cached_results() {
    for code in [5, 6] {
        let mut bytes = calculated_fixture::database(&[Some(999), None, Some(41)]);
        bytes[20] = code;
        let original = bytes.clone();
        let forbid_rows = Rc::new(Cell::new(true));
        let source = NativeSource::new(
            Watching {
                data: Cursor::new(bytes),
                forbid_rows: forbid_rows.clone(),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap();
        let preview = source
            .prepare_calculation("Items", "Score", ParseLimits::default())
            .unwrap();
        assert_eq!(preview.declaration().expression().unwrap(), "[Id]*2");
        assert_eq!(preview.declaration().data_type(), ScalarType::Int32);
        assert_eq!(preview.dependencies(), ["Id"]);
        assert!(
            source
                .unavailable_columns("Items")
                .unwrap()
                .iter()
                .any(|name| name.as_str() == "Score")
        );

        forbid_rows.set(false);
        let result = preview.execute(ExecutionOptions::default()).unwrap();
        assert_eq!(result.columns(), ["Recomputed"]);
        assert_eq!(result.rows().len(), 3);
        // The stored caches are 999, NULL and 41; recomputation uses ordinary Id 1, 2 and 3.
        let output = result
            .rows()
            .iter()
            .map(|row| match row.as_slice() {
                [Value::Int32(value)] => i64::from(*value),
                [Value::Int64(value)] => *value,
                _ => panic!("unexpected arithmetic result carrier"),
            })
            .collect::<Vec<_>>();
        assert_eq!(output, [2, 4, 6]);
        let cached = read_stored_int32_calculation(
            &mut Cursor::new(original),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(cached.values(), [Some(999), None, Some(41)]);
    }
}

#[test]
fn corrupt_unselected_cache_payload_does_not_become_a_preview_input() {
    let mut bytes = calculated_fixture::database(&[Some(999)]);
    let page = PARENT_DATA_PAGE * PAGE_BYTES;
    let row = u16::from_le_bytes(bytes[page + 14..page + 16].try_into().unwrap()) as usize;
    bytes[page + row + 6] = 1;
    assert!(
        read_stored_int32_calculation(
            &mut Cursor::new(bytes.clone()),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false,
        )
        .is_err()
    );
    let forbid_rows = Rc::new(Cell::new(true));
    let source = NativeSource::new(
        Watching {
            data: Cursor::new(bytes),
            forbid_rows: forbid_rows.clone(),
        },
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap();
    let preview = source
        .prepare_calculation("Items", "Score", ParseLimits::default())
        .unwrap();
    forbid_rows.set(false);
    assert_eq!(
        preview
            .execute(ExecutionOptions::default())
            .unwrap()
            .rows()
            .len(),
        1
    );
}

#[test]
fn unsupported_active_nonlocal_and_calculated_dependencies_fail_before_rows() {
    for formula in [
        "DCount('*','Items')",
        "Now()",
        "[Score]*2",
        "[Missing]*2",
        "Items.Id*2",
        "(SELECT Id FROM Items)",
        "[Id]; SELECT 1",
        "Forms!Example!Name",
        "IsNull([Id])",
    ] {
        let source = NativeSource::new(
            Watching {
                data: Cursor::new(calculated_fixture::database_with_expression(
                    &[Some(999)],
                    formula,
                )),
                forbid_rows: Rc::new(Cell::new(true)),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            source
                .prepare_calculation("Items", "Score", ParseLimits::default())
                .is_err(),
            "{formula}"
        );
    }
}

#[test]
fn parser_query_native_limits_and_cancellation_remain_explicit() {
    let bytes = calculated_fixture::database(&[Some(999)]);
    let cancelled = Cell::new(false);
    let cancel = || cancelled.get();
    let source =
        NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &cancel).unwrap();
    assert!(matches!(
        source.prepare_calculation(
            "Items",
            "Score",
            ParseLimits {
                max_bytes: 1,
                ..Default::default()
            }
        ),
        Err(CalculationError::Parse(_))
    ));
    let preview = source
        .prepare_calculation("Items", "Score", ParseLimits::default())
        .unwrap();
    let result = preview.execute(ExecutionOptions {
        limits: QueryLimits {
            steps: 0,
            ..Default::default()
        },
        ..Default::default()
    });
    assert!(matches!(
        result,
        Err(CalculationError::Query(QueryError::Limit(_)))
    ));
    cancelled.set(true);
    assert!(matches!(
        preview.execute(ExecutionOptions::default()),
        Err(CalculationError::Query(QueryError::Cancelled))
    ));
    assert!(matches!(
        source.prepare_calculation("Items", "Score", ParseLimits::default()),
        Err(CalculationError::Query(QueryError::Cancelled))
            | Err(CalculationError::Native(InspectError::Cancelled))
    ));
}

#[test]
fn native_preparation_limits_refuse_without_a_partial_plan() {
    let bytes = calculated_fixture::database(&[Some(999)]);
    let mut exhausted = false;
    for ceiling in [64 * 1024, 128 * 1024, 256 * 1024] {
        if let Ok(source) = NativeSource::new(
            Cursor::new(bytes.clone()),
            CatalogScope::new(1),
            ResourceBudget::new(ceiling, 16384, 1_000_000),
            &|| false,
        ) {
            exhausted |= matches!(
                source.prepare_calculation("Items", "Score", ParseLimits::default()),
                Err(CalculationError::Native(InspectError::Limit(_)))
                    | Err(CalculationError::Query(QueryError::Limit(_)))
            );
        }
    }
    assert!(exhausted);
}

#[test]
fn preview_retains_natural_carriers_and_literal_sql_stays_inert() {
    let formula = "'abc' & ';SELECT Now()'";
    let source = NativeSource::new(
        Cursor::new(calculated_fixture::database_with_expression(
            &[Some(41)],
            formula,
        )),
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap();
    let preview = source
        .prepare_calculation("Items", "Score", ParseLimits::default())
        .unwrap();
    assert!(preview.dependencies().is_empty());
    assert_eq!(preview.declaration().data_type(), ScalarType::Int32);
    assert_eq!(
        preview.execute(ExecutionOptions::default()).unwrap().rows(),
        [vec![Value::Text("abc;SELECT Now()".into())]]
    );
    // A malformed/stale database can declare a carrier inconsistent with its
    // formula. The explicit preview does not coerce or certify stored results.
}

#[test]
fn decimal_declarations_keep_recomputed_outputs_separate_from_stored_scale_and_cache() {
    for code in [5, 6] {
        let payload = vec![14, 0, 4, 128, 0, 0, 0, 0, 57, 48, 0, 0, 0, 0, 0, 0];
        let bytes = scalar_fixture::database(code, 16, 39, false, &[Some(payload)]);
        let forbid_rows = Rc::new(Cell::new(true));
        let source = NativeSource::new(
            Watching {
                data: Cursor::new(bytes.clone()),
                forbid_rows: forbid_rows.clone(),
            },
            CatalogScope::new(1),
            budget(),
            &|| false,
        )
        .unwrap();
        let preview = source
            .prepare_calculation("Items", "Score", ParseLimits::default())
            .unwrap();
        assert_eq!(preview.declaration().data_type(), ScalarType::Decimal);
        assert_eq!(
            preview.declaration().column().decimal_parameters(),
            Some((0, 0))
        );
        assert_eq!(preview.dependencies(), ["Id"]);

        forbid_rows.set(false);
        let result = preview.execute(ExecutionOptions::default()).unwrap();
        assert_eq!(result.columns(), ["Recomputed"]);
        assert!(matches!(
            result.rows()[0].as_slice(),
            [Value::Int32(2)] | [Value::Int64(2)]
        ));
        let stored = read_stored_calculation(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            stored.values(),
            [Value::Decimal(Decimal::new(-12345, 4).unwrap())]
        );
    }
}

#[test]
fn memo_declarations_do_not_read_or_reinterpret_corrupt_cached_storage() {
    let bytes = memo_fixture::database(6, false, &[Some(memo_fixture::inline(&[255]))], &[]);
    let forbid_rows = Rc::new(Cell::new(true));
    let source = NativeSource::new(
        Watching {
            data: Cursor::new(bytes.clone()),
            forbid_rows: forbid_rows.clone(),
        },
        CatalogScope::new(1),
        budget(),
        &|| false,
    )
    .unwrap();
    let preview = source
        .prepare_calculation("Items", "Score", ParseLimits::default())
        .unwrap();
    assert_eq!(preview.declaration().data_type(), ScalarType::Text);
    assert_eq!(preview.declaration().column().storage_length(), 0);
    assert_eq!(preview.dependencies(), ["Id"]);

    forbid_rows.set(false);
    let result = preview.execute(ExecutionOptions::default()).unwrap();
    assert!(matches!(
        result.rows()[0].as_slice(),
        [Value::Int32(2)] | [Value::Int64(2)]
    ));
    assert!(
        read_stored_calculation(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false,
        )
        .is_err()
    );
}

#[path = "calculation_preview/predicates.rs"]
mod predicates;
