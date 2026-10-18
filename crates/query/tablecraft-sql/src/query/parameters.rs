use super::DataType;
use crate::select::Name;
/// A typed PARAMETERS declaration, without prompting or binding values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// Parameter name, which may contain a prompt in brackets.
    pub name: Name,
    /// Written Access type and optional integer arguments.
    pub data_type: DataType,
}
