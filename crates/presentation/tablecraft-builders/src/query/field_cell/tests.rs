use super::*;
use crate::query::{DesignField, DesignFieldMetadata, DesignTotal};
use tablecraft_schema::ScalarType;

#[test]
fn field_text_exposes_aliases_without_confusing_colons_with_identifiers() {
    let mut column = DesignColumn::field(7, "CustomerID".into());
    column.alias = Some("CustomerCount".into());
    column.total = DesignTotal::Count;
    assert_eq!(
        field_cell_text(&column).unwrap(),
        "CustomerCount: CustomerID"
    );
    assert_eq!(
        parse_field_cell(&field_cell_text(&column).unwrap()).unwrap(),
        (Some("CustomerCount".into()), "CustomerID")
    );

    column.alias = Some("Heading: Count".into());
    column.expression = DesignExpression::Field(DesignField {
        source_id: 7,
        column: "Field:Value".into(),
    });
    let rendered = field_cell_text(&column).unwrap();
    assert_eq!(rendered, "[Heading: Count]: [Field:Value]");
    let (alias, operand) = parse_field_cell(&rendered).unwrap();
    assert_eq!(alias.as_deref(), Some("Heading: Count"));
    assert_eq!(operand, "[Field:Value]");
    assert_eq!(column.total, DesignTotal::Count);
}

#[test]
fn plain_fields_borrow_and_explicit_expressions_keep_their_exact_text() {
    let mut column = DesignColumn::field(7, "CustomerID".into());
    assert!(matches!(
        field_cell_text(&column).unwrap(),
        Cow::Borrowed("CustomerID")
    ));
    column.expression = DesignExpression::Expression("IIf(True, 'a:b', 'c')".into());
    column.alias = Some("Result".into());
    assert_eq!(
        field_cell_text(&column).unwrap(),
        "Result: IIf(True, 'a:b', 'c')"
    );
    let (alias, operand) = parse_field_cell("[Result]: IIf(").unwrap();
    assert_eq!(alias.as_deref(), Some("Result"));
    assert_eq!(operand, "IIf(");
    column.expression = DesignExpression::Field(DesignField {
        source_id: 7,
        column: "(".repeat(ParseLimits::MAX_DEPTH + 1),
    });
    column.alias = None;
    assert_eq!(
        field_cell_text(&column).unwrap(),
        format!("[{}]", "(".repeat(ParseLimits::MAX_DEPTH + 1))
    );
    column.expression = DesignExpression::Expression("IIf(True, 'a:b', 'c')".into());
    column.alias = Some("[".into());
    assert_eq!(
        field_cell_text(&column).unwrap(),
        "[: IIf(True, 'a:b', 'c')"
    );
}

#[test]
fn alias_edits_rebind_quoted_and_qualified_fields_to_the_real_source() {
    let sources = [
        DesignSource {
            id: 7,
            table: "Contacts".into(),
            alias: Some("Parent".into()),
        },
        DesignSource {
            id: 8,
            table: "contacts".into(),
            alias: Some("Child".into()),
        },
    ];
    let metadata = [DesignTableMetadata {
        table: "Contacts".into(),
        fields: vec![DesignFieldMetadata {
            name: "Field:Value".into(),
            scalar_type: Some(ScalarType::Int32),
        }],
    }];
    let (alias, operand) = parse_field_cell("[Chosen: Heading]: [Field:Value]").unwrap();
    assert_eq!(alias.as_deref(), Some("Chosen: Heading"));
    assert_eq!(
        edit_field_operand(&sources, &metadata, Some(8), operand.into()).unwrap(),
        DesignExpression::Field(DesignField {
            source_id: 8,
            column: "Field:Value".into()
        })
    );
    assert_eq!(
        edit_field_operand(
            &sources,
            &metadata,
            Some(8),
            "[Parent].[Field:Value]".into()
        )
        .unwrap(),
        DesignExpression::Field(DesignField {
            source_id: 7,
            column: "Field:Value".into()
        })
    );
    assert_eq!(
        edit_field_operand(&sources, &metadata, Some(8), "Fie".into()).unwrap(),
        DesignExpression::Field(DesignField {
            source_id: 8,
            column: "Fie".into()
        })
    );
    assert!(matches!(
        edit_field_operand(&sources, &metadata, None, "[Field:Value]".into()).unwrap(),
        DesignExpression::Expression(_)
    ));
    assert!(matches!(
        edit_field_operand(
            &sources,
            &metadata,
            Some(8),
            "[Missing].[Field:Value]".into()
        )
        .unwrap(),
        DesignExpression::Expression(_)
    ));
    let oversized = "x".repeat(ParseLimits::MAX_BYTES + 1);
    assert!(matches!(
        parse_field_cell(&oversized),
        Err(QueryDesignError::Limit)
    ));
}

#[test]
fn incomplete_cells_stay_unqualified_and_parser_limits_never_become_incomplete_input() {
    for text in [
        "[Alias",
        "[Alias]:",
        "Label: IIf(",
        "Label: 'unfinished",
        "1 +",
    ] {
        assert!(!field_cell_complete(text).unwrap(), "{text}");
    }
    for text in [
        "ID",
        "[A:B]",
        "[Alias]: [A:B]",
        "Label: IIf(True, 1, 2)",
        "*",
    ] {
        assert!(field_cell_complete(text).unwrap(), "{text}");
    }
    let deep = format!(
        "Alias: {}1{}",
        "(".repeat(ParseLimits::MAX_DEPTH + 1),
        ")".repeat(ParseLimits::MAX_DEPTH + 1)
    );
    assert!(matches!(
        field_cell_complete(&deep),
        Err(QueryDesignError::Limit)
    ));
}
