use super::DataType;
use crate::select::Name;
/// A PARAMETERS declaration, without prompting or binding values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// Parameter name, which may contain a prompt in brackets.
    pub name: Name,
    /// Written Access type and optional integer arguments; VALUE denotes Variant.
    pub data_type: DataType,
}
