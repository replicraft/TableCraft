//! Complete metadata/payload cache selection for aligned attachment members.
use super::payload::DecodedFiles;
use super::{AttachmentCache, NativeField, NativeSource, payload, query_error};
use tablecraft_ace::{
    NativeColumn, NativeReadSource, NativeSortOrder, read_attachment_column_with_limits,
    read_attachment_metadata_with_limits,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::{QueryError, source::validate_projection};

impl<R: NativeReadSource> NativeSource<'_, R> {
    pub(super) fn load_members(
        &self,
        table: &str,
        selected: &[usize],
    ) -> Result<Vec<NativeField<'_>>, QueryError> {
        if selected.is_empty() {
            return Ok(Vec::new());
        }
        let members = self.attachment_members(table)?;
        validate_projection(selected, members.len())?;
        let entry = self.entry(table)?;
        let cache = entry.attachments.get().ok_or(QueryError::Source)?;
        self.budget
            .try_borrow_mut()
            .map_err(|_| QueryError::Source)?
            .charge_allocation(selected.len() as u64, size_of::<NativeField<'_>>() as u64)
            .map_err(|error| query_error(error.into()))?;
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(selected.len())
            .map_err(|_| QueryError::Allocation)?;
        for index in selected {
            if (self.cancelled)() {
                return Err(QueryError::Cancelled);
            }
            let member = &members[*index];
            let owner = members
                .iter()
                .position(|other| {
                    other
                        .collection()
                        .as_str()
                        .eq_ignore_ascii_case(member.collection().as_str())
                })
                .ok_or(QueryError::Source)?;
            self.budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?
                .charge(Resource::Steps, selected.len() as u64)
                .map_err(|error| query_error(error.into()))?;
            let payload_requested = selected.iter().any(|selected| {
                let other = &members[*selected];
                other
                    .collection()
                    .as_str()
                    .eq_ignore_ascii_case(member.collection().as_str())
                    && other
                        .column()
                        .name()
                        .eq_ignore_ascii_case(payload::FILE_DATA)
            });
            if !payload_requested && cache.snapshots[owner].stored.get().is_none() {
                let metadata = &cache.snapshots[owner].metadata;
                if metadata.get().is_none() {
                    let mut source = self
                        .source
                        .try_borrow_mut()
                        .map_err(|_| QueryError::Source)?;
                    let mut budget = self
                        .budget
                        .try_borrow_mut()
                        .map_err(|_| QueryError::Source)?;
                    let snapshot = read_attachment_metadata_with_limits(
                        &mut *source,
                        self.scope,
                        &entry.name,
                        member.collection().as_str(),
                        self.snapshot_limits,
                        &mut budget,
                        self.cancelled,
                    )
                    .map_err(query_error)?;
                    self.validate_attachment_members(
                        cache,
                        member.collection().as_str(),
                        snapshot.child_snapshot().schema().columns(),
                        |name| snapshot.metadata_text_sort_order(name),
                        &mut budget,
                    )?;
                    metadata.set(snapshot).map_err(|_| QueryError::Source)?;
                }
                fields.push(NativeField::AttachmentMetadata(
                    metadata.get().ok_or(QueryError::Source)?,
                    member.column().name(),
                ));
                continue;
            }
            let cell = &cache.snapshots[owner].stored;
            if cell.get().is_none() {
                let mut source = self
                    .source
                    .try_borrow_mut()
                    .map_err(|_| QueryError::Source)?;
                let mut budget = self
                    .budget
                    .try_borrow_mut()
                    .map_err(|_| QueryError::Source)?;
                let snapshot = read_attachment_column_with_limits(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    member.collection().as_str(),
                    self.snapshot_limits,
                    &mut budget,
                    self.cancelled,
                )
                .map_err(query_error)?;
                self.validate_attachment_members(
                    cache,
                    member.collection().as_str(),
                    snapshot.links().child_snapshot().schema().columns(),
                    |name| snapshot.metadata_text_sort_order(name),
                    &mut budget,
                )?;
                cell.set(snapshot).map_err(|_| QueryError::Source)?;
            }
            let snapshot = cell.get().ok_or(QueryError::Source)?;
            if member
                .column()
                .name()
                .eq_ignore_ascii_case(payload::FILE_DATA)
            {
                let decoded = &cache.snapshots[owner].decoded;
                if decoded.get().is_none() {
                    let mut budget = self
                        .budget
                        .try_borrow_mut()
                        .map_err(|_| QueryError::Source)?;
                    let files = DecodedFiles::read(snapshot, &mut budget, self.cancelled)?;
                    decoded.set(files).map_err(|_| QueryError::Source)?;
                }
                fields.push(NativeField::AttachmentData(
                    snapshot,
                    decoded.get().ok_or(QueryError::Source)?,
                ));
            } else {
                fields.push(NativeField::Attachment(snapshot, member.column().name()));
            }
        }
        Ok(fields)
    }

    fn validate_attachment_members(
        &self,
        cache: &AttachmentCache,
        collection: &str,
        fields: &[NativeColumn],
        mut order: impl FnMut(&str) -> Option<NativeSortOrder>,
        budget: &mut ResourceBudget,
    ) -> Result<(), QueryError> {
        for (position, declared) in cache.members.iter().enumerate() {
            if (self.cancelled)() {
                return Err(QueryError::Cancelled);
            }
            budget
                .charge(Resource::Steps, 1)
                .map_err(|error| query_error(error.into()))?;
            if !declared
                .collection()
                .as_str()
                .eq_ignore_ascii_case(collection)
            {
                continue;
            }
            let field = fields
                .iter()
                .find(|field| field.name().eq_ignore_ascii_case(declared.column().name()))
                .ok_or(QueryError::SchemaChanged)?;
            if field.scalar_type() != Some(declared.column().data_type())
                || order(declared.column().name()).map(crate::collation::identity)
                    != cache.text_collations[position]
            {
                return Err(QueryError::SchemaChanged);
            }
        }
        Ok(())
    }
}
