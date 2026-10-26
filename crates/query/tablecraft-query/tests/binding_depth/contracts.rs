use super::support::{COMPOUND, Data, extended, prepared, syntax};
use tablecraft_core::values::Value;
use tablecraft_expressions::functions::conversions::text::NumberFormat;
use tablecraft_query::{
    BindingDepth, ExecutionOptions, PreparationOptions, QueryError, prepare, prepare_with_options,
};
use tablecraft_sql::{
    query::{ReadBody, Statement},
    select::TableSource,
};

#[test]
fn depth_selection_is_validated_without_changing_the_existing_default() {
    assert_eq!(BindingDepth::default().get(), 16);
    assert_eq!(BindingDepth::default().to_string(), "16");
    for depth in [0, 33, usize::MAX] {
        assert_eq!(BindingDepth::new(depth), Err(QueryError::StructureLimit));
    }
    assert_eq!(BindingDepth::new(1).unwrap().get(), 1);
    assert_eq!(BindingDepth::new(32).unwrap().get(), 32);
}

#[test]
fn compound_queries_require_opt_in_and_capture_their_limit_in_the_plan() {
    let source = Data::new(&[]);
    assert_eq!(
        prepare(&syntax(COMPOUND), &source).err(),
        Some(QueryError::StructureLimit)
    );
    assert_eq!(source.scans.get(), 0);
    let plan = prepared(COMPOUND, &source);
    assert_eq!(plan.binding_depth(), extended());
    assert_eq!(source.scans.get(), 0);

    for selected in [1, 16, 32] {
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    binding_depth: BindingDepth::new(selected).unwrap(),
                    number_format: Some(NumberFormat::Invariant),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows().len(), 2);
        assert_eq!(result.rows()[0][0], Value::Int16(14));
        assert_eq!(result.rows()[0][2], Value::Int64(2));
        assert_eq!(result.rows()[1][0], Value::Int16(5));
        assert_eq!(result.rows()[1][2], Value::Int64(1));
    }
}

#[test]
fn extended_depth_does_not_remove_absolute_depth_or_node_guards() {
    let source = Data::new(&[]);
    let mut query = syntax("SELECT K FROM T");
    for _ in 0..40 {
        let Statement::Read(inner) = query.statement else {
            unreachable!();
        };
        let mut outer = syntax("SELECT K FROM (SELECT K FROM T) D");
        let Statement::Read(read) = &mut outer.statement else {
            unreachable!();
        };
        let ReadBody::Select(select) = &mut read.body else {
            unreachable!();
        };
        let TableSource::Derived { query: target, .. } = &mut select.from[0] else {
            unreachable!();
        };
        *target = inner;
        query = outer;
    }
    let options = PreparationOptions {
        binding_depth: extended(),
        ..Default::default()
    };
    assert_eq!(
        prepare_with_options(&query, &source, options).err(),
        Some(QueryError::StructureLimit)
    );

    let expression = "CInt(1)+CInt(2)+CInt(3)+CInt(4)+CInt(5)+CInt(6)+CInt(7)";
    // Build an owned query from individually parsed projections so this tests
    // the binder independently of the parser's smaller token ceiling.
    let mut wide = syntax("SELECT 1");
    let mut projections = Vec::new();
    for index in 0..128 {
        let item = syntax(&format!("SELECT {expression} AS C{index}"));
        let Statement::Read(read) = item.statement else {
            unreachable!();
        };
        let ReadBody::Select(mut select) = read.body else {
            unreachable!();
        };
        projections.push(select.projection.pop().unwrap());
    }
    let Statement::Read(read) = &mut wide.statement else {
        unreachable!();
    };
    let ReadBody::Select(select) = &mut read.body else {
        unreachable!();
    };
    select.projection = projections;
    assert_eq!(
        prepare_with_options(&wide, &source, options).err(),
        Some(QueryError::StructureLimit)
    );
    assert_eq!(source.scans.get(), 0);
}
