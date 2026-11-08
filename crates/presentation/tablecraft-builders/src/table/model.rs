mod description;
mod primary;
mod properties;

pub use properties::{DraftDefaultMode, DraftNumberSize};

use super::{MAX_DRAFT_FIELDS, TableDraftError, valid_name};
use properties::FieldProperties;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DraftFieldId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftFieldType {
    AutoNumberIncrement,
    ShortText,
    Number,
    LargeInteger,
    YesNo,
    Currency,
    DateTime,
}

pub struct DraftField {
    id: DraftFieldId,
    name: String,
    kind: DraftFieldType,
    primary: bool,
    protected: bool,
    description: Option<String>,
    properties: FieldProperties,
}

impl DraftField {
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

    pub fn is_protected(&self) -> bool {
        self.protected
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

pub struct TableDraft {
    fields: Vec<DraftField>,
    revision: u64,
    next_field: u64,
    modified: bool,
    design: bool,
}

impl TableDraft {
    pub fn initial() -> Self {
        Self {
            fields: vec![DraftField {
                id: DraftFieldId(1),
                name: "ID".into(),
                kind: DraftFieldType::AutoNumberIncrement,
                primary: true,
                protected: true,
                description: None,
                properties: FieldProperties::new(DraftFieldType::AutoNumberIncrement),
            }],
            revision: 0,
            next_field: 2,
            modified: false,
            design: false,
        }
    }

    /// Fresh manual Design starts with no invented ID or primary declaration.
    pub fn design() -> Self {
        Self {
            fields: Vec::new(),
            revision: 0,
            next_field: 1,
            modified: false,
            design: true,
        }
    }

    pub fn is_design(&self) -> bool {
        self.design
    }

    pub fn fields(&self) -> &[DraftField] {
        &self.fields
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn retained_bytes(&self) -> usize {
        size_of::<Self>()
            + self.fields.capacity() * size_of::<DraftField>()
            + self
                .fields
                .iter()
                .map(|field| {
                    field.name.capacity()
                        + field.description.as_ref().map_or(0, String::capacity)
                        + field.properties.retained_bytes()
                })
                .sum::<usize>()
    }

    pub fn add_field(&mut self, kind: DraftFieldType) -> Result<DraftFieldId, TableDraftError> {
        if kind == DraftFieldType::AutoNumberIncrement {
            return Err(TableDraftError::ProtectedField);
        }
        if self.fields.len() == MAX_DRAFT_FIELDS {
            return Err(TableDraftError::Limit);
        }
        let next_revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;
        let next_field = self
            .next_field
            .checked_add(1)
            .ok_or(TableDraftError::Limit)?;
        let name = super::next_field_name(self.fields.iter().map(|field| field.name.as_str()))?;
        self.insert_field(kind, name, next_revision, next_field)
    }

    /// Add one explicitly named ordinary field as an atomic draft mutation.
    /// Invalid names, duplicate names and exhausted identities leave the draft
    /// unchanged; no temporary generated name or intermediate field is published.
    pub fn add_named_field(
        &mut self,
        kind: DraftFieldType,
        name: &str,
    ) -> Result<DraftFieldId, TableDraftError> {
        if kind == DraftFieldType::AutoNumberIncrement {
            return Err(TableDraftError::ProtectedField);
        }
        if self.fields.len() >= MAX_DRAFT_FIELDS {
            return Err(TableDraftError::Limit);
        }
        if !valid_name(name)
            || self
                .fields
                .iter()
                .any(|field| field.name.eq_ignore_ascii_case(name))
        {
            return Err(TableDraftError::InvalidName);
        }
        let next_revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;
        let next_field = self
            .next_field
            .checked_add(1)
            .ok_or(TableDraftError::Limit)?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(name.len())
            .map_err(|_| TableDraftError::Allocation)?;
        owned.push_str(name);

        self.insert_field(kind, owned, next_revision, next_field)
    }

    fn insert_field(
        &mut self,
        kind: DraftFieldType,
        name: String,
        next_revision: u64,
        next_field: u64,
    ) -> Result<DraftFieldId, TableDraftError> {
        self.fields
            .try_reserve_exact(1)
            .map_err(|_| TableDraftError::Allocation)?;
        let id = DraftFieldId(self.next_field);
        self.fields.push(DraftField {
            id,
            name,
            kind,
            primary: false,
            protected: false,
            description: None,
            properties: FieldProperties::new(kind),
        });
        self.next_field = next_field;
        self.revision = next_revision;
        self.modified = true;
        Ok(id)
    }

    /// Remove one unsaved field without changing surviving identities or order.
    /// The initial AutoNumber remains protected; removed identifiers are never reused.
    pub fn remove_field(&mut self, id: DraftFieldId) -> Result<(), TableDraftError> {
        let position = self
            .fields
            .iter()
            .position(|field| field.id == id)
            .ok_or(TableDraftError::MissingField)?;
        if self.fields[position].protected {
            return Err(TableDraftError::ProtectedField);
        }
        let revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;

        self.fields.remove(position);
        self.revision = revision;
        self.modified = true;
        Ok(())
    }

    /// Change only an unsaved field declaration; no pending input is converted.
    /// Existing identifiers, names and positions stay stable. Required/default
    /// choices remain exact; Short Text capacity and Allow Zero Length retain
    /// hidden caches but are exposed only when Short Text applies again.
    pub fn set_field_type(
        &mut self,
        id: DraftFieldId,
        kind: DraftFieldType,
    ) -> Result<(), TableDraftError> {
        let position = self
            .fields
            .iter()
            .position(|field| field.id == id)
            .ok_or(TableDraftError::MissingField)?;
        if self.fields[position].protected {
            return Err(TableDraftError::ProtectedField);
        }
        match kind {
            DraftFieldType::AutoNumberIncrement => return Err(TableDraftError::ProtectedField),
            DraftFieldType::ShortText
            | DraftFieldType::Number
            | DraftFieldType::LargeInteger
            | DraftFieldType::YesNo
            | DraftFieldType::Currency
            | DraftFieldType::DateTime => {}
        }
        if self.fields[position].kind == kind {
            return Ok(());
        }
        if self.fields[position].primary && !self.fields[position].supports_primary_type(kind) {
            return Err(TableDraftError::InvalidPrimaryKey);
        }
        let revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;

        self.fields[position].kind = kind;
        self.revision = revision;
        self.modified = true;
        Ok(())
    }

    pub fn rename_field(&mut self, id: DraftFieldId, name: &str) -> Result<(), TableDraftError> {
        let position = self
            .fields
            .iter()
            .position(|field| field.id == id)
            .ok_or(TableDraftError::MissingField)?;
        if self.fields[position].protected {
            return Err(TableDraftError::ProtectedField);
        }
        if !valid_name(name)
            || self
                .fields
                .iter()
                .enumerate()
                .any(|(index, field)| index != position && field.name.eq_ignore_ascii_case(name))
        {
            return Err(TableDraftError::InvalidName);
        }
        if self.fields[position].name == name {
            return Ok(());
        }
        let revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;
        let mut replacement = String::new();
        replacement
            .try_reserve_exact(name.len())
            .map_err(|_| TableDraftError::Allocation)?;
        replacement.push_str(name);
        self.fields[position].name = replacement;
        self.revision = revision;
        self.modified = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
