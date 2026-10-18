use super::*;
pub(super) fn view(p: &mut Parser<'_>, context: &ParseContext) -> Result<CreateView, ParseError> {
    let name = identifier(p)?;
    let fields = if p.peek_token().token == Token::LParen {
        names(p)?
    } else {
        Vec::new()
    };
    expect(p, W::AS)?;
    if !matches!(p.peek_token().token, Token::Word(ref word) if word.keyword == W::SELECT) {
        return Err(ParseError::at(
            p,
            K::Syntax,
            "view requires SELECT without declarations",
        ));
    }
    Ok(CreateView {
        name,
        fields,
        query: super::super::queries::read(p, context)?,
    })
}
pub(super) fn procedure(
    p: &mut Parser<'_>,
    context: &ParseContext,
    create: bool,
) -> Result<Procedure, ParseError> {
    let name = identifier(p)?;
    let parameters = if p.peek_token().token == Token::SemiColon
        || matches!(p.peek_token().token, Token::Word(ref word) if word.keyword == W::AS)
    {
        Vec::new()
    } else {
        super::super::parameters::list(p)?
    };
    let as_separator = p.parse_keyword(W::AS);
    if !as_separator {
        expect_token(p, Token::SemiColon)?;
    }
    let body = Box::new(super::super::queries::query(p, context)?);
    Ok(Procedure {
        name,
        parameters,
        create,
        as_separator,
        body,
    })
}
