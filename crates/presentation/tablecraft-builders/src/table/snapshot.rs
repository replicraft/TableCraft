//! Immutable, bounded declaration captured at an explicit save request.
use super::{DraftFieldId, DraftFieldType, TableDraft, TableDraftError, valid_name};

#[derive(PartialEq, Eq)]
pub struct SnapshotField {
    id: DraftFieldId,
    name: String,
    kind: DraftFieldType,
    primary: bool,
}

impl SnapshotField {
    pub fn id(&self) -> DraftFieldId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> DraftFieldType {
        self.kind
    }

    pub fn is_primary(&self) -> bool {
        self.primary
    }
}

/// Exact owned declaration. It carries no native file, table or record authority.
/// Equality compares declarations exactly; it does not provide SQL value equality.
#[derive(PartialEq, Eq)]
pub struct TableDraftSnapshot {
    table_name: String,
    revision: u64,
    fields: Vec<SnapshotField>,
}

impl TableDraftSnapshot {
    pub fn table_name(&self) -> &str {
        &self.table_name
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn fields(&self) -> &[SnapshotField] {
        &self.fields
    }
}

fn copy_name(name: &str) -> Result<String, TableDraftError> {
    let mut result = String::new();
    result
        .try_reserve_exact(name.len())
        .map_err(|_| TableDraftError::Allocation)?;
    result.push_str(name);
    Ok(result)
}

impl TableDraft {
    pub fn snapshot(&self, table_name: &str) -> Result<TableDraftSnapshot, TableDraftError> {
        if !valid_name(table_name) {
            return Err(TableDraftError::InvalidName);
        }
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(self.fields().len())
            .map_err(|_| TableDraftError::Allocation)?;
        for field in self.fields() {
            fields.push(SnapshotField {
                id: field.id(),
                name: copy_name(field.name())?,
                kind: field.kind(),
                primary: field.is_primary(),
            });
        }
        Ok(TableDraftSnapshot {
            table_name: copy_name(table_name)?,
            revision: self.revision(),
            fields,
        })
    }
}
