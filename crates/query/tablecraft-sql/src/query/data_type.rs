/// Recognized Access type spelling and its syntactic arguments.
/// Synonyms retain their spelling; this is not a stored-field layout or type binder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataType {
    /// Uppercase type token; the parser checks a documented Access type allowlist.
    pub name: String,
    /// Optional length or precision/scale arguments, retained without coercion.
    pub arguments: Vec<u32>,
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)?;
        if !self.arguments.is_empty() {
            f.write_str("(")?;
            for (i, argument) in self.arguments.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{argument}")?;
            }
            f.write_str(")")?;
        }
        Ok(())
    }
}
