//! Immutable, bounded declaration captured at an explicit save request.
use super::{
    DraftDefaultMode, DraftFieldId, DraftFieldType, TableDraft, TableDraftError, valid_name,
};

#[derive(PartialEq, Eq)]
pub struct SnapshotField {
    id: DraftFieldId,
    name: String,
    kind: DraftFieldType,
    primary: bool,
    short_text_units: Option<u16>,
    required: Option<bool>,
    allow_zero_length: Option<bool>,
    default_mode: DraftDefaultMode,
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

    pub fn short_text_units(&self) -> Option<u16> {
        self.short_text_units
    }

    pub fn required(&self) -> Option<bool> {
        self.required
    }

    pub fn allow_zero_length(&self) -> Option<bool> {
        self.allow_zero_length
    }

    pub fn default_mode(&self) -> &DraftDefaultMode {
        &self.default_mode
    }
}

/// Exact owned declaration. It carries no native file, table or record authority.
/// Only properties applicable to the captured type are included; an editor's
/// hidden Short Text policy caches remain outside this effective declaration.
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

    pub fn retained_bytes(&self) -> usize {
        size_of::<Self>()
            + self.table_name.capacity()
            + self.fields.capacity() * size_of::<SnapshotField>()
            + self
                .fields
                .iter()
                .map(|field| field.name.capacity() + field.default_mode.retained_bytes())
                .sum::<usize>()
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
                short_text_units: field.short_text_units(),
                required: field.required(),
                allow_zero_length: field.allow_zero_length(),
                default_mode: field.default_mode().try_copy()?,
            });
        }
        Ok(TableDraftSnapshot {
            table_name: copy_name(table_name)?,
            revision: self.revision(),
            fields,
        })
    }
}
