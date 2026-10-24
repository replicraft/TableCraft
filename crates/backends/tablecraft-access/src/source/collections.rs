//! Aligned named metadata views over separately cached complete child snapshots.
mod load;
mod payload;
use super::{NativeSource, query_error};
use payload::DecodedFiles;
use std::cell::OnceCell;
use tablecraft_ace::{
    AttachmentColumn, AttachmentMetadataColumn, ComplexParentLink, MultivalueColumn,
    inspect_attachment_declarations,
};
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_query::{
    QueryError,
    source::{MultivalueList, MultivalueVisitor, TextCollationId, validate_projection},
};
use tablecraft_schema::CollectionMember;

pub(super) struct AttachmentCache {
    members: Vec<CollectionMember>,
    text_collations: Vec<Option<TextCollationId>>,
    // Only the first member of each collection owns a complete snapshot.
    snapshots: Vec<AttachmentSnapshot>,
}
struct AttachmentSnapshot {
    metadata: OnceCell<AttachmentMetadataColumn>,
    stored: OnceCell<AttachmentColumn>,
    decoded: OnceCell<DecodedFiles>,
}
enum NativeField<'a> {
    Value(&'a MultivalueColumn),
    Attachment(&'a AttachmentColumn, &'a str),
    AttachmentMetadata(&'a AttachmentMetadataColumn, &'a str),
    AttachmentData(&'a AttachmentColumn, &'a DecodedFiles),
}
impl NativeField<'_> {
    fn parents(&self) -> &[ComplexParentLink] {
        match self {
            Self::Value(column) => column.links().parents(),
            Self::Attachment(column, _) | Self::AttachmentData(column, _) => {
                column.links().parents()
            }
            Self::AttachmentMetadata(column, _) => column.parents(),
        }
    }
    fn value(&self, parent: usize, child: usize) -> Option<&Value> {
        match self {
            Self::Value(column) => column.value(parent, child),
            Self::Attachment(column, member) => column.metadata_value(parent, child, member),
            Self::AttachmentMetadata(column, member) => {
                column.metadata_value(parent, child, member)
            }
            Self::AttachmentData(_, files) => files.get(parent, child),
        }
    }
}
struct NativeList<'a> {
    field: &'a NativeField<'a>,
    parent: usize,
    length: usize,
}
impl MultivalueList for NativeList<'_> {
    fn len(&self) -> usize {
        self.length
    }
    fn get(&self, child: usize) -> Option<&Value> {
        self.field.value(self.parent, child)
    }
}
impl<R: tablecraft_ace::NativeReadSource> NativeSource<'_, R> {
    pub(super) fn attachment_members(
        &self,
        table: &str,
    ) -> Result<&[CollectionMember], QueryError> {
        if !self.collections {
            return Ok(&[]);
        }
        if self.text_policy_scope.required_order().is_some() {
            return Err(QueryError::Unsupported);
        }
        self.schema_columns(table)?;
        let entry = self.entry(table)?;
        if entry.attachments.get().is_none() {
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let unavailable = &entry.columns.get().ok_or(QueryError::Source)?.unavailable;
            let declarations = if unavailable.is_empty() {
                Vec::new()
            } else {
                let mut source = self
                    .source
                    .try_borrow_mut()
                    .map_err(|_| QueryError::Source)?;
                inspect_attachment_declarations(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    &mut budget,
                    self.cancelled,
                )
                .map_err(query_error)?
            };
            budget
                .charge_allocation(
                    declarations.len() as u64,
                    (size_of::<CollectionMember>() + size_of::<Option<TextCollationId>>()) as u64,
                )
                .map_err(|error| query_error(error.into()))?;
            let mut members = Vec::new();
            let mut text_collations = Vec::new();
            members
                .try_reserve_exact(declarations.len())
                .map_err(|_| QueryError::Allocation)?;
            text_collations
                .try_reserve_exact(declarations.len())
                .map_err(|_| QueryError::Allocation)?;
            for declaration in declarations {
                if (self.cancelled)() {
                    return Err(QueryError::Cancelled);
                }
                budget.charge(Resource::Steps, 1)?;
                text_collations.push(
                    declaration
                        .text_sort_order()
                        .map(crate::collation::identity),
                );
                members.push(declaration.into_member());
            }
            if self.decoded_attachments {
                payload::declare(&mut members, &mut budget, self.cancelled)?;
                // Decoded FileData declarations are appended and remain Binary.
                let extra = members.len() - text_collations.len();
                budget
                    .charge_allocation(extra as u64, size_of::<Option<TextCollationId>>() as u64)
                    .map_err(|error| query_error(error.into()))?;
                text_collations
                    .try_reserve_exact(extra)
                    .map_err(|_| QueryError::Allocation)?;
                text_collations.resize(members.len(), None);
            }
            if members.iter().any(|member| {
                !unavailable.iter().any(|name| {
                    name.as_str()
                        .eq_ignore_ascii_case(member.collection().as_str())
                })
            }) {
                return Err(QueryError::SchemaChanged);
            }
            budget
                .charge_allocation(members.len() as u64, size_of::<AttachmentSnapshot>() as u64)
                .map_err(|error| query_error(error.into()))?;
            let mut snapshots = Vec::new();
            snapshots
                .try_reserve_exact(members.len())
                .map_err(|_| QueryError::Allocation)?;
            snapshots.extend((0..members.len()).map(|_| AttachmentSnapshot {
                metadata: OnceCell::new(),
                stored: OnceCell::new(),
                decoded: OnceCell::new(),
            }));
            entry
                .attachments
                .set(AttachmentCache {
                    members,
                    text_collations,
                    snapshots,
                })
                .map_err(|_| QueryError::Source)?;
        }
        Ok(&entry.attachments.get().ok_or(QueryError::Source)?.members)
    }
    pub(super) fn attachment_text_collations(
        &self,
        table: &str,
    ) -> Result<&[Option<TextCollationId>], QueryError> {
        if !self.collections {
            return Ok(&[]);
        }
        self.attachment_members(table)?;
        Ok(&self
            .entry(table)?
            .attachments
            .get()
            .ok_or(QueryError::Source)?
            .text_collations)
    }
    pub(super) fn member_scan(
        &self,
        table: &str,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        let _span = tracing::debug_span!("native.collection.scan").entered();
        if values.is_empty() && members.is_empty() {
            return Err(QueryError::Unsupported);
        }
        validate_projection(indices, self.schema_columns(table)?.len())?;
        if self.entry(table)?.index.is_some() {
            return Err(QueryError::Unsupported);
        }
        let selected = self.load_values(table, values)?;
        let mut fields = self.load_members(table, members)?;
        self.budget
            .try_borrow_mut()
            .map_err(|_| QueryError::Source)?
            .charge_allocation(values.len() as u64, size_of::<NativeField<'_>>() as u64)
            .map_err(|error| query_error(error.into()))?;
        fields
            .try_reserve_exact(values.len())
            .map_err(|_| QueryError::Allocation)?;
        // Legacy Value views precede named members, matching the source contract.
        for (position, column) in selected.into_iter().enumerate() {
            fields.insert(position, NativeField::Value(column));
        }
        let parents = fields.first().ok_or(QueryError::Source)?.parents().len();
        if fields.iter().any(|field| field.parents().len() != parents) {
            return Err(QueryError::Source);
        }
        let mut ordinal = 0;
        self.projected_scan(table, indices, None, &mut |row| {
            if (self.cancelled)() {
                return Err(QueryError::Cancelled);
            }
            if ordinal >= parents {
                return Err(QueryError::Source);
            }
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            budget
                .charge(Resource::Steps, fields.len() as u64 + 1)
                .map_err(|error| query_error(error.into()))?;
            budget
                .charge_allocation(
                    fields.len() as u64,
                    (size_of::<NativeList<'_>>() + size_of::<&dyn MultivalueList>()) as u64,
                )
                .map_err(|error| query_error(error.into()))?;
            let mut lists = Vec::new();
            lists
                .try_reserve_exact(fields.len())
                .map_err(|_| QueryError::Allocation)?;
            for field in &fields {
                lists.push(NativeList {
                    field,
                    parent: ordinal,
                    length: field
                        .parents()
                        .get(ordinal)
                        .ok_or(QueryError::Source)?
                        .child_count(),
                });
            }
            let mut views = Vec::new();
            views
                .try_reserve_exact(lists.len())
                .map_err(|_| QueryError::Allocation)?;
            views.extend(lists.iter().map(|list| list as &dyn MultivalueList));
            drop(budget);
            visitor(row, &views)?;
            ordinal += 1;
            Ok(())
        })?;
        if ordinal != parents {
            return Err(QueryError::Source);
        }
        Ok(())
    }
}
