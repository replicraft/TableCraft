//! Total roles preserve grouping, clause phase and exact importable SQL.
use super::{draft, metadata};
use crate::query::{
    DesignColumn, DesignExpression, DesignField, DesignFieldMetadata, DesignSort, DesignTotal,
    QueryDesignError, build, build_ast, import, import_unbound, validate_totals_off,
};
use tablecraft_schema::ScalarType;
use tablecraft_sql::{
    ParseLimits, parse_query,
    query::{ReadBody, Statement},
    select::{BinaryOperator, Expression, FunctionArgument},
};

fn grouped() -> crate::query::QueryDesignDraft {
    let mut draft = draft();
    draft.totals = true;
    draft.columns[1].total = DesignTotal::Sum;
    draft.columns[1].alias = Some("Total".into());
    draft.columns[1].criteria = vec!["> 100".into()];
    draft.columns[1].sort = Some(DesignSort::Descending);
    let mut filter = DesignColumn::field(1, "ID".into());
    filter.total = DesignTotal::Where;
    filter.show = false;
    filter.criteria = vec!["> 1".into()];
    draft.columns.push(filter);
    draft
}

#[test]
fn group_sum_and_where_build_captured_operands_in_their_distinct_phases() {
    let draft = grouped();
    let query = build_ast(&draft, &metadata()).unwrap();
    let Statement::Read(read) = query.statement else {
        panic!("SELECT owner");
    };
    let ReadBody::Select(select) = read.body else {
        panic!("one SELECT");
    };
    assert_eq!(select.group_by.len(), 1);
    assert!(matches!(
        select.selection,
        Some(Expression::Binary {
            operator: BinaryOperator::Greater,
            ..
        })
    ));
    let Some(Expression::Binary {
        operator: BinaryOperator::Greater,
        left,
        ..
    }) = select.having
    else {
        panic!("aggregate HAVING");
    };
    let Expression::Call {
        name, arguments, ..
    } = &*left
    else {
        panic!("captured aggregate");
    };
    assert_eq!(name.as_str(), "Sum");
    assert!(
        matches!(arguments.as_slice(), [FunctionArgument::Expression(Expression::Field(field))] if field.name.as_str()=="Amount")
    );
    assert_eq!(read.order_by.len(), 1);
    assert_eq!(read.order_by[0].expression, *left);
    let sql = build(&draft, &metadata()).unwrap().sql;
    assert!(!sql.split(" FROM ").next().unwrap().contains("ID"));
    assert_eq!(
        parse_query(&sql, ParseLimits::default())
            .unwrap()
            .query()
            .to_string(),
        sql
    );
}

#[test]
fn hidden_group_key_and_count_star_keep_membership_and_null_count_distinct() {
    let mut draft = draft();
    draft.totals = true;
    draft.columns[0].show = false;
    draft.columns[1].total = DesignTotal::Count;
    draft.columns[1].alias = Some("NonNull".into());
    let mut rows = DesignColumn::field(1, "ID".into());
    rows.expression = DesignExpression::Expression("Count(*)".into());
    rows.total = DesignTotal::Expression;
    rows.alias = Some("Rows".into());
    draft.columns.push(rows);
    let sql = build(&draft, &metadata()).unwrap().sql;
    assert!(sql.contains("Count([Customers].[Amount]) AS [NonNull]"));
    assert!(sql.contains("Count(*) AS [Rows]"));
    assert!(sql.contains("GROUP BY [Customers].[Name]"));
    assert!(!sql.split(" FROM ").next().unwrap().contains("[Name]"));
    assert_eq!(import(&sql, &metadata()).unwrap().sql, sql);
}

#[test]
fn phase_local_or_is_preserved_and_cross_phase_alternatives_refuse() {
    let mut draft = grouped();
    draft.columns[2].criteria.clear();
    draft.columns[1].criteria = vec!["> 100".into(), "= 0".into()];
    let sql = build(&draft, &metadata()).unwrap().sql;
    assert!(sql.contains(" HAVING "));
    assert!(sql.contains(" OR "));
    assert!(!sql.contains(" WHERE "));
    draft.columns[2].criteria = vec!["> 1".into()];
    assert!(matches!(
        build(&draft, &metadata()),
        Err(QueryDesignError::Unsupported)
    ));
}

#[test]
fn explicit_off_never_resets_aggregate_roles_or_where_visibility() {
    let mut draft = grouped();
    assert!(validate_totals_off(&draft).is_err());
    draft.totals = false;
    assert!(build(&draft, &metadata()).is_err());
    assert_eq!(draft.columns[1].total, DesignTotal::Sum);
    assert_eq!(draft.columns[1].criteria, ["> 100"]);
    assert_eq!(draft.columns[2].total, DesignTotal::Where);
    draft.totals = true;
    draft.columns[2].show = true;
    assert!(build(&draft, &metadata()).is_err());
    draft.columns[2].show = false;
    draft.columns[2].sort = Some(DesignSort::Ascending);
    assert!(build(&draft, &metadata()).is_err());
    let mut simple = super::draft();
    simple.totals = true;
    assert!(validate_totals_off(&simple).is_ok());
    simple.columns[0].expression = DesignExpression::Expression("Count(*)".into());
    assert!(validate_totals_off(&simple).is_err());
}

#[test]
fn declared_grouped_imports_preserve_hidden_keys_clauses_order_and_case_boundaries() {
    for sql in [
        "PARAMETERS [Min] CURRENCY; SELECT Sum([Customers].[Amount]) AS Total FROM Customers WHERE Customers.ID>1 GROUP BY Customers.Name HAVING Sum(Customers.Amount)>=[Min] ORDER BY Sum(Customers.Amount) DESC",
        "SELECT Customers.Name, SUM(Customers.Amount) AS Total FROM Customers GROUP BY Customers.Name HAVING SUM(Customers.Amount)>100",
        "SELECT Count(*) AS Rows FROM Customers",
        "SELECT Customers.Name FROM Customers GROUP BY Customers.Name",
    ] {
        let imported = import(sql, &metadata()).unwrap_or_else(|error| panic!("{sql}: {error:?}"));
        assert!(imported.draft.totals);
        assert_eq!(
            build(&imported.draft, &metadata()).unwrap().sql,
            imported.sql
        );
    }
    for sql in [
        "SELECT Sum(Amount) AS Total FROM Customers WHERE ID=1 OR ID=2 GROUP BY Name HAVING Sum(Amount)>10 OR Sum(Amount)<0",
        "SELECT DISTINCT Name FROM Customers GROUP BY Name",
        "SELECT Count(*) AS Rows FROM Customers UNION SELECT Count(*) FROM Orders",
        "SELECT Name FROM Customers GROUP BY (SELECT ID FROM Orders)",
    ] {
        assert!(import(sql, &metadata()).is_err(), "{sql}");
    }
    let mut unknown = grouped();
    unknown.columns[1].expression = DesignExpression::Expression("Amount + Missing".into());
    assert!(build(&unknown, &metadata()).is_err());
    assert!(
        import_unbound("SELECT Sum(Amount) FROM Customers")
            .unwrap()
            .totals
    );
}

#[test]
fn evidenced_plain_field_defaults_preserve_explicit_alias_and_expression_choices() {
    for (role, heading) in [
        (DesignTotal::Sum, "SumOfAmount"),
        (DesignTotal::Min, "MinOfAmount"),
        (DesignTotal::Max, "MaxOfAmount"),
    ] {
        let mut draft = draft();
        draft.totals = true;
        draft.columns[1].total = role;
        let built = build(&draft, &metadata()).unwrap();
        assert!(built.sql.contains(&format!(" AS [{heading}]")));
        assert!(draft.columns[1].alias.is_none());
        assert_eq!(import(&built.sql, &metadata()).unwrap().sql, built.sql);
        draft.columns[1].alias = Some("Chosen heading".into());
        let explicit = build(&draft, &metadata()).unwrap();
        assert!(explicit.sql.contains(" AS [Chosen heading]"));
        assert!(!explicit.sql.contains(&format!(" AS [{heading}]")));
    }
    let mut expression = draft();
    expression.totals = true;
    expression.columns[1].total = DesignTotal::Expression;
    expression.columns[1].expression =
        DesignExpression::Expression("Sum([Customers].[Amount])".into());
    assert!(
        !build(&expression, &metadata())
            .unwrap()
            .sql
            .contains("SumOfAmount")
    );
    expression.columns[1].total = DesignTotal::Count;
    expression.columns[1].expression = DesignExpression::Field(DesignField {
        source_id: 1,
        column: "Amount".into(),
    });
    assert!(
        !build(&expression, &metadata())
            .unwrap()
            .sql
            .contains("CountOfAmount")
    );
}

#[test]
fn generated_heading_collisions_refuse_without_invented_suffixes_or_implicit_renaming() {
    let mut draft = draft();
    draft.totals = true;
    draft.columns[1].total = DesignTotal::Sum;
    draft
        .columns
        .push(DesignColumn::field(1, "SumOfAmount".into()));
    let mut metadata = metadata();
    metadata[0].fields.push(DesignFieldMetadata {
        name: "SumOfAmount".into(),
        scalar_type: Some(ScalarType::Currency),
    });
    assert!(matches!(
        build(&draft, &metadata),
        Err(QueryDesignError::Unsupported)
    ));
    assert!(draft.columns[1].alias.is_none());
    draft.columns[1].alias = Some("SelectedSum".into());
    assert!(build(&draft, &metadata).is_ok());
    let original = "SELECT Sum(Customers.Amount) FROM Customers";
    assert!(matches!(
        import(original, &metadata),
        Err(QueryDesignError::Unsupported)
    ));
    assert!(import_unbound(original).is_ok());
}
