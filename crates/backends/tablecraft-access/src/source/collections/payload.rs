//! Explicit decoded binary views; raw metadata snapshots retain wrapped bytes.
use super::super::query_error;
use tablecraft_ace::{AttachmentColumn, AttachmentDecoder};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_query::QueryError;
use tablecraft_schema::{CollectionMember, Column, ScalarType};

pub(super) const FILE_DATA: &str = "FileData";
const COPY_BLOCK_BYTES: usize = 4096;

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}
fn owned(value: &str, budget: &mut ResourceBudget) -> Result<String, QueryError> {
    budget.charge_allocation(value.len() as u64, 1)?;
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| QueryError::Allocation)?;
    result.push_str(value);
    Ok(result)
}
pub(super) fn declare(
    members: &mut Vec<CollectionMember>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryError> {
    let metadata_len = members.len();
    for index in 0..metadata_len {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, index as u64)?;
        let name = members[index].collection().as_str();
        if members[..index]
            .iter()
            .any(|other| other.collection().as_str().eq_ignore_ascii_case(name))
        {
            continue;
        }
        let collection = owned(name, budget)?;
        let column = Column::new(owned(FILE_DATA, budget)?, ScalarType::Binary, true)?;
        budget.charge_allocation(1, size_of::<CollectionMember>() as u64)?;
        members
            .try_reserve_exact(1)
            .map_err(|_| QueryError::Allocation)?;
        members.push(CollectionMember::new(collection, column)?);
    }
    check(budget, cancelled)
}

pub(super) struct DecodedFiles {
    parents: Vec<Vec<Value>>,
}
impl DecodedFiles {
    pub(super) fn get(&self, parent: usize, child: usize) -> Option<&Value> {
        self.parents.get(parent)?.get(child)
    }
    pub(super) fn read(
        snapshot: &AttachmentColumn,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, QueryError> {
        let _span = tracing::debug_span!("native.attachment.query.decode").entered();
        check(budget, cancelled)?;
        let count = snapshot.links().parents().len();
        budget.charge_allocation(count as u64, size_of::<Vec<Value>>() as u64)?;
        let mut parents = Vec::new();
        parents
            .try_reserve_exact(count)
            .map_err(|_| QueryError::Allocation)?;
        let mut decoder = AttachmentDecoder::new(budget, cancelled);
        for parent in 0..count {
            check(decoder.budget_mut(), cancelled)?;
            let attachments = snapshot.attachments(parent).ok_or(QueryError::Source)?;
            decoder
                .budget_mut()
                .charge_allocation(attachments.len() as u64, size_of::<Value>() as u64)?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(attachments.len())
                .map_err(|_| QueryError::Allocation)?;
            for attachment in attachments {
                check(decoder.budget_mut(), cancelled)?;
                let decoded = attachment
                    .map_err(query_error)?
                    .decode_with(&mut decoder)
                    .map_err(query_error)?;
                let value = if let Some(decoded) = decoded {
                    let data = decoded.data();
                    decoder
                        .budget_mut()
                        .charge_allocation(data.len() as u64, 1)?;
                    let mut bytes = Vec::new();
                    bytes
                        .try_reserve_exact(data.len())
                        .map_err(|_| QueryError::Allocation)?;
                    for block in data.chunks(COPY_BLOCK_BYTES) {
                        check(decoder.budget_mut(), cancelled)?;
                        decoder
                            .budget_mut()
                            .charge(Resource::Steps, block.len() as u64)?;
                        bytes.extend_from_slice(block);
                    }
                    Value::Binary(bytes)
                } else {
                    Value::Null
                };
                values.push(value);
            }
            parents.push(values);
        }
        check(decoder.budget_mut(), cancelled)?;
        Ok(Self { parents })
    }
}
