//! Presentation labels never replace original field or query-alias identity.
use crate::workspace::Data;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub caption: Option<String>,
}

impl Column {
    pub fn label(&self) -> &str {
        self.caption
            .as_deref()
            .filter(|caption| !caption.is_empty())
            .unwrap_or(&self.name)
    }
}

impl From<String> for Column {
    fn from(name: String) -> Self {
        Self {
            name,
            caption: None,
        }
    }
}

impl From<&str> for Column {
    fn from(name: &str) -> Self {
        name.to_owned().into()
    }
}

impl Data {
    /// Resolve only original names, never a caption that another field can share.
    pub(crate) fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|column| column.name == name)
    }
}

#[cfg(test)]
mod tests;
