use super::{ParseError, access::name, expression::expect_token};
use crate::query::Parameter;
use sqlparser::{parser::Parser, tokenizer::Token};
pub(super) fn parse(p: &mut Parser<'_>) -> Result<Vec<Parameter>, ParseError> {
    if !super::expression::word(p, "PARAMETERS") {
        return Ok(Vec::new());
    }
    let parameters = list(p)?;
    expect_token(p, Token::SemiColon)?;
    Ok(parameters)
}
pub(super) fn list(p: &mut Parser<'_>) -> Result<Vec<Parameter>, ParseError> {
    let mut parameters = Vec::new();
    loop {
        let name = name(p)?;
        let data_type = super::data_type::parse(p, false)?;
        parameters.push(Parameter { name, data_type });
        if parameters.len() > 255 {
            return Err(ParseError::at(
                p,
                super::ParseErrorKind::Limit,
                "parameter count exceeds 255",
            ));
        }
        if !p.consume_token(&Token::Comma) {
            break;
        }
    }
    Ok(parameters)
}
