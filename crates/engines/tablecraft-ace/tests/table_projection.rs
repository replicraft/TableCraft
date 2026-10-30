#[path = "table_projection/cursor.rs"]
mod cursor;
use std::io::Cursor;
use tablecraft_ace::{InspectError, read_table, read_table_projection};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as fixture;

const PAGE_BYTES: usize = 4096;
const OWNER_PAGE: usize = 10;
const FIRST_FIELD: usize = 63;
const FIELD_BYTES: usize = 25;
const UNKNOWN_TYPE: u8 = 0xff;
fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}

#[test]
fn explicit_projection_preserves_native_order_and_never_qualifies_unselected_values() {
    let bytes = fixture::database();
    let both = read_table_projection(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &["Score", "Id"],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        both.columns()
            .iter()
            .map(|column| column.name())
            .collect::<Vec<_>>(),
        ["Id", "Score"]
    );
    assert_eq!(both.rows()[0], [Value::Int32(1), Value::Int32(10)]);
    for names in [&["Id", "Id"][..], &["missing"][..]] {
        assert!(
            read_table_projection(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                names,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    let mut mixed = bytes.clone();
    mixed[OWNER_PAGE * PAGE_BYTES + FIRST_FIELD + FIELD_BYTES] = UNKNOWN_TYPE;
    assert!(matches!(
        read_table(
            &mut Cursor::new(&mixed),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Unsupported)
    ));
    let selected = read_table_projection(
        &mut Cursor::new(&mixed),
        CatalogScope::new(1),
        "Items",
        &["iD"],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        selected.rows(),
        &[
            vec![Value::Int32(1)],
            vec![Value::Int32(2)],
            vec![Value::Int32(3)]
        ]
    );
    assert!(matches!(
        read_table_projection(
            &mut Cursor::new(&mixed),
            CatalogScope::new(1),
            "Items",
            &["Score"],
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Unsupported)
    ));
    let empty = read_table_projection(
        &mut Cursor::new(&mixed),
        CatalogScope::new(1),
        "Items",
        &[],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(empty.columns().is_empty());
    assert_eq!(empty.rows(), &[vec![], vec![], vec![]]);
    assert!(matches!(
        read_table_projection(
            &mut Cursor::new(&mixed),
            CatalogScope::new(1),
            "Items",
            &[],
            &mut budget(),
            &|| true
        ),
        Err(InspectError::Cancelled)
    ));
    assert!(matches!(
        read_table_projection(
            &mut Cursor::new(&mixed),
            CatalogScope::new(1),
            "Items",
            &[],
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
}
