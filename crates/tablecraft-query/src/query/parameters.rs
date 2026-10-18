use crate::select::Name;
/// A typed PARAMETERS declaration, without prompting or binding values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// Parameter name, which may contain a prompt in brackets.
    pub name: Name,
    /// Written Access type and optional integer arguments.
    pub data_type: ParameterType,
}
/// Recognized Access type spelling and its syntactic arguments.
/// Synonyms retain their spelling; this is not a stored-field layout or type binder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterType {
    /// Uppercase type token; the parser checks a documented Access type allowlist.
    pub name: String,
    /// Optional length or precision/scale arguments, retained without coercion.
    pub arguments: Vec<u32>,
}
