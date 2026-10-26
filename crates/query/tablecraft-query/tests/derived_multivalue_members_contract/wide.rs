use super::support::*;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_query::{
    BindingDepth, PreparationOptions, PreparedQuery, QueryError, QueryValue, prepare_with_options,
};
use tablecraft_sql::{ParseLimits, parse_query};

fn prepare_wide(sql: &str, data: &Data<'_>) -> Result<PreparedQuery, QueryError> {
    let query = parse_query(
        sql,
        ParseLimits {
            max_bytes: ParseLimits::MAX_BYTES,
            max_tokens: ParseLimits::MAX_TOKENS,
            max_depth: ParseLimits::MAX_DEPTH,
        },
    )
    .unwrap();
    prepare_with_options(
        query.query(),
        data,
        PreparationOptions {
            binding_depth: BindingDepth::new(32).unwrap(),
            ..Default::default()
        },
    )
}

#[test]
fn unused_virtual_members_do_not_halve_the_supported_whole_list_projection_width() {
    let data = wide_list_fixture();
    let projection = (0..129)
        .map(|index| format!("A AS X{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!("SELECT * FROM (SELECT {projection} FROM L) Q");
    let plan = prepare_wide(&sql, &data).unwrap();
    assert!(data.scans.borrow().is_empty());
    assert_eq!(plan.output_types().unwrap().len(), 129);
    let result = plan
        .execute_collections(&data, &[], Default::default())
        .unwrap();
    assert_eq!(
        result.rows().len(),
        1,
        "unused virtual members must not expand"
    );
    assert_eq!(result.rows()[0].len(), 129);
    for output in &result.rows()[0] {
        let QueryValue::Multivalue(value) = output else {
            panic!("each logical output must remain a whole present list");
        };
        assert_eq!(value.values(), data.tables[0].rows[0].lists[0]);
    }

    let sql = format!("SELECT Q.X128.Value,Q.X128 FROM (SELECT {projection} FROM L) Q");
    let plan = prepare_wide(&sql, &data).unwrap();
    let result = plan
        .execute_collections(&data, &[], Default::default())
        .unwrap();
    let expected = data.tables[0].rows[0].lists[0]
        .iter()
        .map(|child| vec![stored(child.clone()), list(&data.tables[0].rows[0], 0)])
        .collect::<Vec<_>>();
    assert_rows(&result, &expected);
}

#[test]
fn public_logical_width_limit_remains_256_independent_of_virtual_member_storage() {
    let data = maximum_scalar_fixture();
    let plan = prepare_wide("SELECT * FROM (SELECT * FROM L) Q", &data).unwrap();
    assert!(data.scans.borrow().is_empty());
    assert_eq!(plan.output_types().unwrap().len(), 256);
    let result = plan
        .execute_collections(&data, &[], Default::default())
        .unwrap();
    assert_eq!(result.columns().len(), 256);
    assert_eq!(result.rows().len(), 1);
    for (index, value) in result.rows()[0].iter().enumerate() {
        assert_eq!(
            value,
            &QueryValue::Scalar(ExpressionValue::Value(Value::Int32(index as i32)))
        );
    }

    let data = maximum_scalar_fixture();
    assert_eq!(
        prepare_wide("SELECT *,1 AS Extra FROM L", &data).err(),
        Some(QueryError::StructureLimit)
    );
    assert!(data.scans.borrow().is_empty());
}
