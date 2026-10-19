use super::fixture as base;
use super::*;
#[allow(dead_code)]
#[path = "../decimal_composite_indexes/fixture.rs"]
mod decimal_composite;
#[allow(dead_code)]
#[path = "../decimal_indexes/fixture.rs"]
mod decimal_fixture;
#[allow(dead_code)]
#[path = "../guid_composite_indexes/fixture.rs"]
mod guid_composite;
#[allow(dead_code)]
#[path = "../guid_indexes/fixture.rs"]
mod guid_fixture;
#[allow(dead_code)]
#[path = "../composite_indexes/fixture.rs"]
mod numeric_fixture;

fn compare(bytes: Vec<u8>) {
    let mut budget = ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000);
    let inspection = tablecraft_ace::inspect_table_indexes(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget,
        &|| false,
    )
    .unwrap();
    let index = &inspection.indexes()[0];
    let number = index.columns().last().unwrap().number();
    let field = inspection
        .schema()
        .columns()
        .iter()
        .find(|field| field.number() == number)
        .unwrap()
        .name();
    let full = tablecraft_ace::read_table_index(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        index.name(),
        &mut budget,
        &|| false,
    )
    .unwrap();
    let position = full
        .columns()
        .iter()
        .position(|column| column.name() == field)
        .unwrap();
    let projected = read_table_index_projection(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        index.name(),
        IndexProjection {
            columns: &[field],
            selection: IndexProjectionSelection::All,
        },
        &mut budget,
        &|| false,
    )
    .unwrap();
    assert_eq!(
        projected.rows(),
        full.rows()
            .iter()
            .map(|row| vec![row[position].clone()])
            .collect::<Vec<_>>()
    );
    let empty = read_table_index_projection(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        index.name(),
        IndexProjection {
            columns: &[],
            selection: IndexProjectionSelection::All,
        },
        &mut budget,
        &|| false,
    )
    .unwrap();
    assert!(empty.rows().iter().all(Vec::is_empty));
    assert_eq!(empty.rows().len(), full.rows().len());
}

#[test]
fn projected_key_views_keep_existing_numeric_guid_decimal_and_tuple_profiles() {
    compare(guid_fixture::database(
        5,
        true,
        &[None, Some([0; 16]), Some([0x80; 16]), Some([0xff; 16])],
        0,
    ));
    compare(decimal_fixture::database(
        false,
        28,
        4,
        &[None, Some(-123400), Some(0), Some(999900)],
        0,
    ));
    compare(numeric_fixture::database(
        6,
        &[4, 4, 5],
        &[(1, true), (2, false)],
        &[
            vec![Some(1), None, Some(-50000)],
            vec![Some(2), Some(42), None],
            vec![Some(3), Some(42), Some(12500)],
        ],
        0,
    ));
    use guid_composite::Cell as G;
    compare(guid_composite::database(
        6,
        &[4, 15, 0x13],
        &[(1, false), (2, true)],
        &[
            vec![
                Some(G::Number(1)),
                Some(G::Guid([0x80; 16])),
                Some(G::Number(9007199254740993)),
            ],
            vec![Some(G::Number(2)), None, Some(G::Number(i64::MIN))],
        ],
        0,
    ));
    use decimal_composite::Cell as D;
    compare(decimal_composite::database(
        6,
        &[4, 16, 15],
        &[(1, false), (2, true)],
        &[
            vec![
                Some(D::Number(1)),
                Some(D::Decimal(-123400)),
                Some(D::Guid([0xff; 16])),
            ],
            vec![Some(D::Number(2)), None, Some(D::Guid([0; 16]))],
        ],
        0,
    ));
}
