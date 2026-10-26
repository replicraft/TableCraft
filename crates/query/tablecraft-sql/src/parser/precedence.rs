//! Access binding powers; only relative order matters (see docs/query/SQL_PARSER.md).
pub(super) const IMPLICATION: u8 = 11;
pub(super) const EQUIVALENCE: u8 = 12;
pub(super) const XOR: u8 = 13;
pub(super) const OR: u8 = 14;
pub(super) const AND: u8 = 15;
pub(super) const NOT: u8 = 20;
pub(super) const COMPARISON: u8 = 30;
pub(super) const CONCATENATION: u8 = 40;
pub(super) const ADDITION: u8 = 50;
pub(super) const MODULO: u8 = 60;
pub(super) const INTEGER_DIVISION: u8 = 70;
pub(super) const MULTIPLICATION: u8 = 80;
pub(super) const UNARY_SIGN: u8 = 85;
pub(super) const POWER: u8 = 90;
