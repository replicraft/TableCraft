use super::*;
pub(super) fn create(p: &mut Parser<'_>) -> Result<CreateIndex, ParseError> {
    let unique = p.parse_keyword(W::UNIQUE);
    expect(p, W::INDEX)?;
    let name = identifier(p)?;
    expect(p, W::ON)?;
    let table = identifier(p)?;
    expect_token(p, Token::LParen)?;
    let mut fields = Vec::new();
    loop {
        let name = identifier(p)?;
        let descending = p.parse_keyword(W::DESC);
        if !descending {
            let _ = p.parse_keyword(W::ASC);
        }
        fields.push(IndexField { name, descending });
        if !p.consume_token(&Token::Comma) {
            break;
        }
    }
    expect_token(p, Token::RParen)?;
    let policy = if p.parse_keyword(W::WITH) {
        Some(if p.parse_keyword(W::PRIMARY) {
            IndexPolicy::Primary
        } else if word(p, "DISALLOW") {
            expect(p, W::NULL)?;
            IndexPolicy::DisallowNull
        } else if p.parse_keyword(W::IGNORE) {
            expect(p, W::NULL)?;
            IndexPolicy::IgnoreNull
        } else {
            return Err(ParseError::at(p, K::Syntax, "expected index policy"));
        })
    } else {
        None
    };
    Ok(CreateIndex {
        name,
        table,
        unique,
        fields,
        policy,
    })
}
