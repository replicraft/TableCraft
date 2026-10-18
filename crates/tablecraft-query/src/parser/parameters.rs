use super::{ParseError, access::name, expression::expect_token};
use crate::query::Parameter;
use sqlparser::{parser::Parser, tokenizer::Token};
pub(super) fn parse(p: &mut Parser<'_>) -> Result<Vec<Parameter>, ParseError> {
    let mut parameters = Vec::new();
    if !super::expression::word(p, "PARAMETERS") {
        return Ok(parameters);
    }
    loop {
        let name = name(p)?;
        let data_type = super::data_type::parse(p, false)?;
        parameters.push(Parameter { name, data_type });
        if !p.consume_token(&Token::Comma) {
            break;
        }
    }
    expect_token(p, Token::SemiColon)?;
    Ok(parameters)
}
