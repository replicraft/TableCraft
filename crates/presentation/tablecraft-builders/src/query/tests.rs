use super::*;
use tablecraft_schema::ScalarType;
use tablecraft_sql::{
    ParseLimits, parse_expression_strict, parse_query,
    query::{ReadBody, Statement},
};

fn metadata() -> Vec<DesignTableMetadata> {
    ["Customers", "Orders"]
        .into_iter()
        .map(|table| DesignTableMetadata {
            table: table.into(),
            fields: [
                ("ID", ScalarType::Int32),
                ("Name", ScalarType::Text),
                ("Amount", ScalarType::Currency),
            ]
            .into_iter()
            .map(|(name, kind)| DesignFieldMetadata {
                name: name.into(),
                scalar_type: Some(kind),
            })
            .collect(),
        })
        .collect()
}

fn draft() -> QueryDesignDraft {
    QueryDesignDraft {
        sources: vec![DesignSource {
            id: 1,
            table: "Customers".into(),
            alias: None,
        }],
        joins: Vec::new(),
        columns: vec![
            DesignColumn::field(1, "Name".into()),
            DesignColumn::field(1, "Amount".into()),
        ],
    }
}

#[test]
fn qbe_rows_preserve_hidden_filters_native_shorthand_and_grid_sort_order() {
    let mut draft = draft();
    draft.columns[0].criteria = vec!["New York".into(), "Like 'L*'".into()];
    draft.columns[1].show = false;
    draft.columns[1].sort = Some(DesignSort::Descending);
    draft.columns[1].criteria = vec!["> 1 And < 5".into(), "Between 10 And 20".into()];
    let built = build(&draft, &metadata()).unwrap();
    let parsed = parse_query(&built.sql, ParseLimits::default()).unwrap();
    assert_eq!(parsed.query().to_string(), built.sql);
    assert!(built.sql.contains("'New York'"));
    assert!(built.sql.contains(" OR "));
    assert!(built.sql.contains("BETWEEN 10 AND 20"));
    assert!(built.sql.contains("ORDER BY [Customers].[Amount] DESC"));
    let projection = built.sql.split(" FROM ").next().unwrap();
    assert!(!projection.contains("Amount"));
    assert_eq!(built.draft, draft);
}

#[test]
fn explicit_join_kinds_and_typed_identifiers_do_not_become_sql_fragments() {
    let mut draft = draft();
    draft.sources[0].alias = Some("customer odd name".into());
    draft.sources.push(DesignSource {
        id: 2,
        table: "Orders".into(),
        alias: Some("o".into()),
    });
    draft.joins.push(DesignJoin {
        left: DesignField {
            source_id: 1,
            column: "ID".into(),
        },
        right: DesignField {
            source_id: 2,
            column: "ID".into(),
        },
        kind: DesignJoinKind::Left,
    });
    let built = build(&draft, &metadata()).unwrap();
    assert!(built.sql.contains("LEFT JOIN [Orders] AS [o]"));
    assert!(built.sql.contains("[customer odd name].[ID] = [o].[ID]"));
    let mut invalid = draft.clone();
    invalid.sources[0].alias = Some("x]; DELETE FROM Orders;[".into());
    assert!(build(&invalid, &metadata()).is_err());
    assert_eq!(draft.sources[0].alias.as_deref(), Some("customer odd name"));
}

#[test]
fn supported_sql_round_trips_and_unsupported_clauses_are_not_dropped() {
    for sql in [
        "SELECT Name FROM Customers WHERE Amount > 1 ORDER BY Name DESC",
        "SELECT Customers.*, Orders.Name AS Label FROM Customers INNER JOIN Orders ON Customers.ID = Orders.ID",
        "SELECT Name FROM Customers WHERE (Name = 'A' AND Amount > 1) OR (Name = 'B' AND Amount < 5)",
    ] {
        let built = import(sql, &metadata()).unwrap();
        assert_eq!(build(&built.draft, &metadata()).unwrap().sql, built.sql);
    }
    for sql in [
        "SELECT DISTINCT Name FROM Customers",
        "SELECT TOP 1 Name FROM Customers",
        "SELECT Name FROM Customers GROUP BY Name",
        "SELECT Sum(Amount) FROM Customers",
        "SELECT Name FROM Customers UNION SELECT Name FROM Orders",
        "SELECT Name FROM Customers IN 'other.accdb'",
        "SELECT Name FROM Customers WHERE EXISTS (SELECT ID FROM Orders)",
    ] {
        assert!(import_unbound(sql).is_err(), "{sql}");
    }
}

#[test]
fn limits_and_invalid_joins_refuse_without_mutating_the_old_draft() {
    let old = draft();
    let mut invalid = old.clone();
    invalid.columns[0].criteria = vec!["1".into(); MAX_DESIGN_CRITERIA_ROWS + 1];
    assert!(build(&invalid, &metadata()).is_err());
    invalid = old.clone();
    invalid.columns[0].criteria = vec!["x".repeat(MAX_DESIGN_TEXT_BYTES)];
    assert!(matches!(
        draft_text_bytes(&invalid),
        Err(QueryDesignError::Limit)
    ));
    invalid = old.clone();
    invalid
        .columns
        .iter_mut()
        .for_each(|column| column.show = false);
    assert!(matches!(
        build(&invalid, &metadata()),
        Err(QueryDesignError::NoProjection)
    ));
    invalid = old.clone();
    invalid.columns[0].expression = DesignExpression::AllFields(Some(999));
    invalid.columns[0].show = false;
    assert!(matches!(
        build(&invalid, &metadata()),
        Err(QueryDesignError::InvalidReference)
    ));
    invalid = old.clone();
    invalid.columns = vec![old.columns[0].clone(); MAX_DESIGN_COLUMNS + 1];
    assert!(matches!(
        draft_text_bytes(&invalid),
        Err(QueryDesignError::Limit)
    ));
    invalid = old.clone();
    invalid.joins.push(DesignJoin {
        left: DesignField {
            source_id: 1,
            column: "ID".into(),
        },
        right: DesignField {
            source_id: 999,
            column: "ID".into(),
        },
        kind: DesignJoinKind::Inner,
    });
    assert!(matches!(
        build(&invalid, &metadata()),
        Err(QueryDesignError::InvalidJoins)
    ));
    assert!(build(&old, &metadata()).is_ok());
}

#[test]
fn contextual_criteria_reuse_native_fragment_grammar_and_bare_text_stays_text() {
    for (column, input, expected) in [
        (
            "Amount",
            ">1 And <5",
            "[Customers].[Amount] > 1 AND [Customers].[Amount] < 5",
        ),
        (
            "Amount",
            "Between 1 And 5",
            "[Customers].[Amount] BETWEEN 1 AND 5",
        ),
        ("Amount", "In (1, 2)", "[Customers].[Amount] IN (1, 2)"),
        ("Amount", "Is Null", "[Customers].[Amount] IS NULL"),
        (
            "Amount",
            ">1 Or <0",
            "[Customers].[Amount] > 1 OR [Customers].[Amount] < 0",
        ),
        ("Name", "New York", "[Customers].[Name] = 'New York'"),
        ("Name", "123", "[Customers].[Name] = '123'"),
        ("Name", "Like 'N*'", "[Customers].[Name] LIKE 'N*'"),
    ] {
        let mut draft = draft();
        let index = usize::from(column == "Amount");
        draft.columns[index].criteria.push(input.into());
        let query = build_ast(&draft, &metadata()).unwrap();
        let Statement::Read(read) = query.statement else {
            panic!("read-only design");
        };
        let ReadBody::Select(select) = read.body else {
            panic!("SELECT design");
        };
        assert_eq!(
            select.selection.as_ref(),
            Some(&parse_expression_strict(expected, ParseLimits::default()).unwrap()),
            "{input}"
        );
    }
}
