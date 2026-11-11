//! Access binding powers; only relative order matters (see docs/query/SQL_PARSER.md).
pub(crate) const IMPLICATION: u8 = 11;
pub(crate) const EQUIVALENCE: u8 = 12;
pub(crate) const XOR: u8 = 13;
pub(crate) const OR: u8 = 14;
pub(crate) const AND: u8 = 15;
pub(crate) const NOT: u8 = 20;
pub(crate) const COMPARISON: u8 = 30;
pub(crate) const CONCATENATION: u8 = 40;
pub(crate) const ADDITION: u8 = 50;
pub(crate) const MODULO: u8 = 60;
pub(crate) const INTEGER_DIVISION: u8 = 70;
pub(crate) const MULTIPLICATION: u8 = 80;
pub(crate) const UNARY_SIGN: u8 = 85;
pub(crate) const POWER: u8 = 90;

pub(crate) fn binary(operator: super::BinaryOperator) -> u8 {
    use super::BinaryOperator as B;

    match operator {
        B::Implication => IMPLICATION,
        B::Equivalence => EQUIVALENCE,
        B::Xor => XOR,
        B::Or => OR,
        B::And => AND,
        B::Equal | B::NotEqual | B::Less | B::LessOrEqual | B::Greater | B::GreaterOrEqual => {
            COMPARISON
        }
        B::Concatenate => CONCATENATION,
        B::Add | B::Subtract => ADDITION,
        B::Modulo => MODULO,
        B::IntegerDivide => INTEGER_DIVISION,
        B::Multiply | B::Divide => MULTIPLICATION,
        B::Power => POWER,
    }
}
