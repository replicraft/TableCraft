//! Conservative checked retained-presentation heap accounting, including hidden text.
use super::{MAX_RETAINED_BYTES, session::Session};
use crate::workspace::State;
use crate::{Data, FieldPropertyValue, Schema};

pub(in crate::workspace) struct Counter(Option<usize>);

impl Counter {
    pub(in crate::workspace) fn new(bytes: usize) -> Self {
        Self(Some(bytes))
    }

    pub(in crate::workspace) fn add(&mut self, bytes: usize) {
        self.0 = self.0.and_then(|total| total.checked_add(bytes));
    }

    pub(in crate::workspace) fn vector<T>(&mut self, values: &Vec<T>) {
        self.0 = self.0.and_then(|total| {
            values
                .capacity()
                .checked_mul(size_of::<T>())?
                .checked_add(total)
        });
    }

    pub(in crate::workspace) fn count<T>(&mut self, length: usize) {
        self.0 = self
            .0
            .and_then(|total| length.checked_mul(size_of::<T>())?.checked_add(total));
    }

    pub(in crate::workspace) fn text(&mut self, text: &str) {
        self.add(text.len());
    }

    pub(in crate::workspace) fn string(&mut self, text: &String) {
        self.add(text.capacity());
    }

    pub(in crate::workspace) fn optional(&mut self, text: &Option<String>) {
        if let Some(text) = text {
            self.string(text);
        }
    }

    pub(in crate::workspace) fn finish(self) -> Option<usize> {
        self.0
    }
}

pub(in crate::workspace) fn data(data: &Data) -> Option<usize> {
    let mut count = Counter::new(size_of::<Data>());
    count.string(&data.table);
    count.vector(&data.columns);
    for column in &data.columns {
        count.string(&column.name);
        count.optional(&column.caption);
    }
    count.vector(&data.rows);
    for row in &data.rows {
        count.vector(row);
        for cell in row {
            count.string(&cell.text);
        }
    }
    if let Some(ordinals) = &data.row_ordinals {
        count.add(ordinals.retained_bytes()?);
    }
    count.finish()
}

pub(in crate::workspace) fn schema(schema: &Schema) -> Option<usize> {
    let mut count = Counter::new(size_of::<Schema>());
    count.string(&schema.table);
    count.vector(&schema.fields);
    for field in &schema.fields {
        count.string(&field.name);
        count.optional(&field.scalar_type);
        count.optional(&field.display_type);
        count.optional(&field.field_size);
        count.vector(&field.properties);
        for property in &field.properties {
            if let FieldPropertyValue::Text(text) = &property.value {
                count.string(text);
            }
        }
    }
    count.finish()
}

impl Session {
    pub(super) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        match self {
            Self::Table {
                data: page,
                schema: fields,
                filter,
                totals,
                records,
                error,
                ..
            } => {
                if let Some(page) = page {
                    count.add(data(page)?);
                }
                if let Some(fields) = fields {
                    count.add(schema(fields)?);
                }
                count.add(filter.retained_bytes()?);
                count.add(totals.retained_bytes()?);
                count.add(records.retained_bytes()?);
                count.optional(error);
            }
            Self::Saved { state, error } => {
                count.add(state.retained_bytes()?);
                count.optional(error);
            }
            Self::Sql {
                data: page,
                error,
                design,
                ..
            } => {
                count.add(design.retained_bytes()?);
                if let Some(page) = page {
                    count.add(data(page)?);
                }
                count.optional(error);
            }
        }
        count.finish()
    }
}

impl super::Objects {
    pub(super) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<super::Objects>());
        count.vector(&self.entries);
        if let Some(target) = &self.navigation {
            match &target.kind {
                super::ObjectKind::Table { name } | super::ObjectKind::SavedQuery { name, .. } => {
                    count.string(name)
                }
                super::ObjectKind::Sql => {}
            }
        }
        for entry in &self.entries {
            match &entry.tab.target.kind {
                super::ObjectKind::Table { name } | super::ObjectKind::SavedQuery { name, .. } => {
                    count.string(name)
                }
                super::ObjectKind::Sql => {}
            }
            if entry.session.is_some() {
                count.add(entry.retained_bytes?);
            }
        }
        count.finish()
    }
}

impl State {
    pub(in crate::workspace) fn object_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(self.objects.retained_bytes()?);
        if let Some(page) = &self.data {
            count.add(data(page)?);
        }
        if let Some(fields) = &self.schema {
            count.add(schema(fields)?);
        }
        if let Some(page) = &self.query_data {
            count.add(data(page)?);
        }
        count.string(&self.sql);
        count.add(self.saved_query.retained_bytes()?);
        count.add(self.table_filter.retained_bytes()?);
        count.add(self.totals.retained_bytes()?);
        count.add(self.query_design.retained_bytes()?);
        count.add(self.records.retained_bytes()?);
        count.optional(&self.error);
        count.finish()
    }

    pub(in crate::workspace) fn admit_object_schema(&mut self, value: &Schema) -> bool {
        let admitted = self
            .object_bytes()
            .and_then(|bytes| bytes.checked_add(schema(value)?))
            .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES);
        if !admitted {
            self.error = Some(self.localizer.text("objects-memory-limit"));
        }
        admitted
    }

    pub(in crate::workspace) fn admit_object_bytes(&mut self, additional: Option<usize>) -> bool {
        let admitted = self
            .object_bytes()
            .and_then(|bytes| bytes.checked_add(additional?))
            .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES);
        if !admitted {
            self.error = Some(self.localizer.text("objects-memory-limit"));
        }
        admitted
    }

    pub(in crate::workspace) fn admit_object_data(&mut self, value: &Data) -> bool {
        let admitted = self
            .object_bytes()
            .and_then(|bytes| bytes.checked_add(data(value)?))
            .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES);
        if !admitted {
            self.error = Some(self.localizer.text("objects-memory-limit"));
        }
        admitted
    }
}
