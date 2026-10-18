use super::*;
pub(super) fn create(p: &mut Parser<'_>, temporary: bool) -> Result<CreateTable, ParseError> {
    let name = identifier(p)?;
    expect_token(p, Token::LParen)?;
    let mut elements = Vec::new();
    loop {
        elements.push(
                    if matches!(p.peek_token().token, Token::Word(ref word) if word.keyword == W::CONSTRAINT) {
                        TableElement::Constraint(constraint::parse(p, false)?)
                    } else {
                        TableElement::Column(column(p, true)?)
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
pub(super) fn alter(p: &mut Parser<'_>) -> Result<AlterTable, ParseError> {
    expect(p, W::TABLE)?;
    let name = identifier(p)?;
    let operation = if p.parse_keyword(W::ADD) {
        if p.parse_keyword(W::COLUMN) {
            AlterOperation::AddColumn(column(p, false)?)
        } else {
            AlterOperation::AddConstraint(constraint::parse(p, false)?)
        }
    } else if p.parse_keyword(W::ALTER) {
        expect(p, W::COLUMN)?;
        AlterOperation::AlterColumn {
            name: identifier(p)?,
            data_type: data_type::parse(p, true)?,
        }
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
fn column(p: &mut Parser<'_>, compression_allowed: bool) -> Result<Column, ParseError> {
    let name = identifier(p)?;
    let data_type = data_type::parse(p, true)?;
    let not_null = if p.parse_keyword(W::NOT) {
        expect(p, W::NULL)?;
        true
    } else {
        false
    };
    let compression = if compression_allowed && p.parse_keyword(W::WITH) {
        if !word(p, "COMPRESSION") && !word(p, "COMP") {
            return Err(ParseError::at(
                p,
                K::Syntax,
                "expected compression attribute",
            ));
        }
        true
    } else {
        false
    };
    let constraint = if matches!(p.peek_token().token, Token::Word(ref w) if matches!(w.keyword, W::CONSTRAINT | W::PRIMARY | W::UNIQUE | W::REFERENCES))
    {
        Some(constraint::parse(p, true)?)
    } else {
        None
    };
    Ok(Column {
        name,
        data_type,
        not_null,
        compression,
        constraint,
    })
}
