use super::*;
pub(super) fn create(
    p: &mut Parser<'_>,
    temporary: bool,
    context: &ParseContext,
) -> Result<CreateTable, ParseError> {
    let name = identifier(p)?;
    expect_token(p, Token::LParen)?;
    let mut elements = Vec::new();
    loop {
        elements.push(
                    if matches!(p.peek_token().token, Token::Word(ref word) if word.keyword == W::CONSTRAINT) {
                        TableElement::Constraint(constraint::parse(p, false, context)?)
                    } else {
                        TableElement::Column(column(p, true, context)?)
                    },
                );
        if !p.consume_token(&Token::Comma) {
            break;
        }
    }
    expect_token(p, Token::RParen)?;
    if !elements
        .iter()
        .any(|e| matches!(e, TableElement::Column(_)))
    {
        return Err(ParseError::at(
            p,
            K::Syntax,
            "table requires at least one field",
        ));
    }
    Ok(CreateTable {
        name,
        temporary,
        elements,
    })
}
pub(super) fn alter(p: &mut Parser<'_>, context: &ParseContext) -> Result<AlterTable, ParseError> {
    expect(p, W::TABLE)?;
    let name = identifier(p)?;
    let operation = if p.parse_keyword(W::ADD) {
        if p.parse_keyword(W::COLUMN) {
            AlterOperation::AddColumn(column(p, false, context)?)
        } else {
            AlterOperation::AddConstraint(constraint::parse(p, false, context)?)
        }
    } else if p.parse_keyword(W::ALTER) {
        expect(p, W::COLUMN)?;
        AlterOperation::AlterColumn(column(p, false, context)?)
    } else if p.parse_keyword(W::DROP) {
        if p.parse_keyword(W::COLUMN) {
            AlterOperation::DropColumn(identifier(p)?)
        } else {
            expect(p, W::CONSTRAINT)?;
            AlterOperation::DropConstraint(identifier(p)?)
        }
    } else {
        return Err(ParseError::at(p, K::Syntax, "expected table alteration"));
    };
    Ok(AlterTable { name, operation })
}
fn column(
    p: &mut Parser<'_>,
    compression_allowed: bool,
    context: &ParseContext,
) -> Result<Column, ParseError> {
    let mut column = Column {
        name: identifier(p)?,
        data_type: data_type::parse(p, true)?,
        not_null: false,
        compression: false,
        default_value: None,
        constraint: None,
    };
    loop {
        if p.parse_keyword(W::NOT) {
            expect(p, W::NULL)?;
            if column.not_null {
                return Err(ParseError::at(
                    p,
                    K::Syntax,
                    "duplicate NOT NULL annotation",
                ));
            }
            column.not_null = true;
        } else if p.parse_keyword(W::WITH) {
            if !compression_allowed || column.compression {
                return Err(ParseError::at(
                    p,
                    K::Syntax,
                    "unsupported or repeated compression annotation",
                ));
            }
            if !word(p, "COMPRESSION") && !word(p, "COMP") {
                return Err(ParseError::at(
                    p,
                    K::Syntax,
                    "expected compression attribute",
                ));
            }
            column.compression = true;
        } else if p.parse_keyword(W::DEFAULT) {
            if column.default_value.is_some() {
                return Err(ParseError::at(p, K::Syntax, "duplicate DEFAULT annotation"));
            }
            column.default_value = Some(default::parse(p, context)?);
        } else if matches!(p.peek_token().token, Token::Word(ref word) if matches!(word.keyword, W::CONSTRAINT | W::PRIMARY | W::UNIQUE | W::REFERENCES | W::CHECK))
        {
            if column.constraint.is_some() {
                return Err(ParseError::at(p, K::Syntax, "duplicate column constraint"));
            }
            column.constraint = Some(constraint::parse(p, true, context)?);
        } else {
            break;
        }
    }
    Ok(column)
}
