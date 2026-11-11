//! Aligned named stored-member views over separately cached complete child snapshots.
mod indexed;
mod load;
mod payload;
mod recordsets;
use super::{NativeSource, query_error};
use payload::DecodedFiles;
use std::cell::OnceCell;
use tablecraft_ace::{
    AttachmentColumn, AttachmentMetadataColumn, ComplexParentLink, MultivalueColumn, ReadProfile,
    VersionHistoryColumn, inspect_attachment_declarations, inspect_header,
    inspect_version_history_declarations,
};
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_db::{MultivalueList, ProjectedRow, validate_projection};
use tablecraft_query::{
    QueryError, SourceFailure,
    source::{MultivalueVisitor, TextCollationId},
};
use tablecraft_schema::{CollectionMember, SchemaName};

pub(super) struct CollectionCache {
    members: Vec<CollectionMember>,
    attachment_member_count: usize,
    recordsets: OnceCell<recordsets::Recordsets>,
    text_collations: Vec<Option<TextCollationId>>,
    history_memos: Vec<Option<SchemaName>>,
    // Only the first member of each collection owns a complete snapshot.
    snapshots: Vec<CollectionSnapshot>,
}
struct CollectionSnapshot {
    metadata: OnceCell<AttachmentMetadataColumn>,
    stored: OnceCell<AttachmentColumn>,
    decoded: OnceCell<DecodedFiles>,
    history: OnceCell<VersionHistoryColumn>,
}

impl CollectionCache {
    pub(super) fn clear_decoded(&mut self) {
        for snapshot in &mut self.snapshots {
            snapshot.decoded.take();
        }
    }
}

enum NativeField<'a> {
    Value(&'a MultivalueColumn),
    Attachment(&'a AttachmentColumn, &'a str),
    AttachmentMetadata(&'a AttachmentMetadataColumn, &'a str),
    AttachmentData(&'a AttachmentColumn, &'a DecodedFiles),
    History(&'a VersionHistoryColumn, &'a str),
}
impl NativeField<'_> {
    fn owner(&self) -> &tablecraft_ace::TableSchema {
        match self {
            Self::Value(column) => column.links().schema().owner(),
            Self::Attachment(column, _) | Self::AttachmentData(column, _) => {
                column.links().schema().owner()
            }
            Self::AttachmentMetadata(column, _) => column.schema().owner(),
            Self::History(column, _) => column.links().schema().owner(),
        }
    }

    fn parents(&self) -> &[ComplexParentLink] {
        match self {
            Self::Value(column) => column.links().parents(),
            Self::Attachment(column, _) | Self::AttachmentData(column, _) => {
                column.links().parents()
            }
            Self::AttachmentMetadata(column, _) => column.parents(),
            Self::History(column, _) => column.links().parents(),
        }
    }
    fn value(&self, parent: usize, child: usize) -> Option<&Value> {
        match self {
            Self::Value(column) => column.value(parent, child),
            Self::Attachment(column, member) => column.stored_value(parent, child, member),
            Self::AttachmentMetadata(column, member) => {
                column.metadata_value(parent, child, member)
            }
            Self::AttachmentData(_, files) => files.get(parent, child),
            Self::History(column, member) => column.member_value(parent, child, member),
        }
    }
}
type ParentVisitor<'a> =
    dyn FnMut(ProjectedRow<'_>, &[&dyn MultivalueList], usize) -> Result<(), QueryError> + 'a;

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
    pub(super) fn named_members(&self, table: &str) -> Result<&[CollectionMember], QueryError> {
        if !self.collections {
            return Ok(&[]);
        }
        if self.text_policy_scope.required_order().is_some() {
            return Err(QueryError::Unsupported);
        }
        self.schema_columns(table)?;
        let entry = self.entry(table)?;
        if entry.collections.get().is_none() {
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::SourceFailure(SourceFailure::Busy))?;
            let unavailable = &entry.columns.get().ok_or(QueryError::Source)?.unavailable;
            let profile = if unavailable.is_empty() {
                None
            } else {
                let mut source = self
                    .source
                    .try_borrow_mut()
                    .map_err(|_| QueryError::SourceFailure(SourceFailure::Busy))?;
                Some(
                    inspect_header(&mut *source, &mut budget, self.cancelled)
                        .map_err(query_error)?
                        .read_profile()
                        .map_err(query_error)?,
                )
            };
            // Attachment and history inspectors have distinct qualified native
            // profiles. Selecting an unavailable metadata capability is not a
            // source error and must not disable unrelated scalar or Value reads.
            // Every error from an inspector that actually runs still propagates.
            let declarations = if matches!(
                profile,
                Some(ReadProfile::Ace2007 | ReadProfile::Ace2016 | ReadProfile::Ace2019)
            ) {
                let mut source = self
                    .source
                    .try_borrow_mut()
                    .map_err(|_| QueryError::SourceFailure(SourceFailure::Busy))?;
                inspect_attachment_declarations(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    &mut budget,
                    self.cancelled,
                )
                .map_err(query_error)?
            } else {
                Vec::new()
            };
            budget
                .charge_allocation(
                    declarations.len() as u64,
                    (size_of::<CollectionMember>() + size_of::<Option<TextCollationId>>()) as u64,
                )
                .map_err(|error| query_error(error.into()))?;
            let mut members = Vec::new();
            let mut text_collations = Vec::new();
            let mut history_memos = Vec::new();
            members
                .try_reserve_exact(declarations.len())
                .map_err(|_| QueryError::SourceFailure(SourceFailure::Allocation))?;
            text_collations
                .try_reserve_exact(declarations.len())
                .map_err(|_| QueryError::SourceFailure(SourceFailure::Allocation))?;
            for declaration in declarations {
                if (self.cancelled)() {
                    return Err(QueryError::Cancelled);
                }
                budget
                    .charge(Resource::Steps, 1)
                    .map_err(crate::source::query_limit)?;
                text_collations.push(
                    declaration
                        .text_sort_order()
                        .map(crate::collation::identity),
                );
                members.push(declaration.into_member());
            }
            let attachment_member_count = members.len();
            // FileData declarations above apply only to attachment collections.
            // Histories use their actual hidden field and native member names.
            budget
                .charge_allocation(members.len() as u64, size_of::<Option<SchemaName>>() as u64)
                .map_err(|error| query_error(error.into()))?;
            history_memos
                .try_reserve_exact(members.len())
                .map_err(|_| QueryError::SourceFailure(SourceFailure::Allocation))?;
            history_memos.resize(members.len(), None);
            if matches!(
                profile,
                Some(
                    ReadProfile::Ace2007
                        | ReadProfile::Ace2010
                        | ReadProfile::Ace2016
                        | ReadProfile::Ace2019
                )
            ) {
                let mut source = self
                    .source
                    .try_borrow_mut()
                    .map_err(|_| QueryError::SourceFailure(SourceFailure::Busy))?;
                let histories = inspect_version_history_declarations(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    &mut budget,
                    self.cancelled,
                )
                .map_err(query_error)?;
                budget
                    .charge_allocation(
                        histories.len() as u64,
                        (size_of::<CollectionMember>()
                            + size_of::<Option<TextCollationId>>()
                            + size_of::<Option<SchemaName>>()) as u64,
                    )
                    .map_err(|error| query_error(error.into()))?;
                members
                    .try_reserve_exact(histories.len())
                    .map_err(|_| QueryError::SourceFailure(SourceFailure::Allocation))?;
                text_collations
                    .try_reserve_exact(histories.len())
                    .map_err(|_| QueryError::SourceFailure(SourceFailure::Allocation))?;
                history_memos
                    .try_reserve_exact(histories.len())
                    .map_err(|_| QueryError::SourceFailure(SourceFailure::Allocation))?;
                for declaration in histories {
                    if (self.cancelled)() {
                        return Err(QueryError::Cancelled);
                    }
                    budget
                        .charge(Resource::Steps, 1)
                        .map_err(crate::source::query_limit)?;
                    let (member, memo, order) = declaration.into_parts();
                    members.push(member);
                    text_collations.push(order.map(crate::collation::identity));
                    history_memos.push(Some(memo));
                }
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
                .charge_allocation(members.len() as u64, size_of::<CollectionSnapshot>() as u64)
                .map_err(|error| query_error(error.into()))?;
            let mut snapshots = Vec::new();
            snapshots
                .try_reserve_exact(members.len())
                .map_err(|_| QueryError::SourceFailure(SourceFailure::Allocation))?;
            snapshots.extend((0..members.len()).map(|_| CollectionSnapshot {
                metadata: OnceCell::new(),
                stored: OnceCell::new(),
                decoded: OnceCell::new(),
                history: OnceCell::new(),
            }));
            entry
                .collections
                .set(CollectionCache {
                    members,
                    attachment_member_count,
                    recordsets: OnceCell::new(),
                    text_collations,
                    history_memos,
                    snapshots,
                })
                .map_err(|_| QueryError::Source)?;
        }
        Ok(&entry.collections.get().ok_or(QueryError::Source)?.members)
    }
    pub(super) fn member_text_collations(
        &self,
        table: &str,
    ) -> Result<&[Option<TextCollationId>], QueryError> {
        if !self.collections {
            return Ok(&[]);
        }
        self.named_members(table)?;
        Ok(&self
            .entry(table)?
            .collections
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
        self.member_scan_with_parent(
            table,
            indices,
            values,
            members,
            &mut |row, lists, _parent| visitor(row, lists),
        )
    }

    fn member_scan_with_parent(
        &self,
        table: &str,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        visitor: &mut ParentVisitor<'_>,
    ) -> Result<(), QueryError> {
        let _span = tracing::debug_span!("native.collection.scan").entered();
        if values.is_empty() && members.is_empty() {
            return Err(QueryError::Unsupported);
        }
        validate_projection(indices, self.schema_columns(table)?.len())?;
        if self.entry(table)?.index.is_some() {
            return self.indexed_member_scan_with_parent(table, indices, values, members, visitor);
        }
        let fields = self.load_fields(table, values, members)?;
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
                .map_err(|_| QueryError::SourceFailure(SourceFailure::Busy))?;
            budget
                .charge(Resource::Steps, fields.len() as u64 + 1)
                .map_err(crate::source::query_limit)?;
            budget
                .charge_allocation(
                    fields.len() as u64,
                    (size_of::<NativeList<'_>>() + size_of::<&dyn MultivalueList>()) as u64,
                )
                .map_err(|error| query_error(error.into()))?;
            let mut lists = Vec::new();
            lists
                .try_reserve_exact(fields.len())
                .map_err(|_| QueryError::SourceFailure(SourceFailure::Allocation))?;
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
                .map_err(|_| QueryError::SourceFailure(SourceFailure::Allocation))?;
            views.extend(lists.iter().map(|list| list as &dyn MultivalueList));
            drop(budget);
            visitor(row, &views, ordinal)?;
            ordinal += 1;
            Ok(())
        })?;
        if ordinal != parents {
            return Err(QueryError::Source);
        }
        Ok(())
    }
}
