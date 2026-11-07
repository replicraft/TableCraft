use super::{MAX_DRAFT_FIELDS, TableDraftError, valid_name};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DraftFieldId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftFieldType {
    AutoNumberIncrement,
    ShortText,
    LongInteger,
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
}

pub struct TableDraft {
    fields: Vec<DraftField>,
    revision: u64,
    next_field: u64,
    modified: bool,
}

impl TableDraft {
    pub fn initial() -> Self {
        Self {
            fields: vec![DraftField {
                id: DraftFieldId(1),
                name: "ID".into(),
                kind: DraftFieldType::AutoNumberIncrement,
                primary: true,
            }],
            revision: 0,
            next_field: 2,
            modified: false,
        }
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
                .map(|field| field.name.capacity())
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
        let mut suffix = 1;
        let name = loop {
            let candidate = format!("Field{suffix}");
            if !self
                .fields
                .iter()
                .any(|field| field.name.eq_ignore_ascii_case(&candidate))
            {
                break candidate;
            }
            suffix += 1;
        };
        self.fields
            .try_reserve_exact(1)
            .map_err(|_| TableDraftError::Allocation)?;
        let id = DraftFieldId(self.next_field);
        self.fields.push(DraftField {
            id,
            name,
            kind,
            primary: false,
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
        if self.fields[position].primary {
            return Err(TableDraftError::ProtectedField);
        }
        let revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;

        self.fields.remove(position);
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
        if self.fields[position].primary {
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
