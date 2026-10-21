use super::Runtime;
use crate::{
    QueryError,
    source::{MultivalueList, ProjectedRow},
};
use tablecraft_core::values::Value;
use tablecraft_schema::{CollectionMember, Column};
mod expansion;
mod parent;

impl Runtime<'_> {
    pub(super) fn collection_requests<'a>(
        &mut self,
        selected: &'a [usize],
        values: usize,
    ) -> Result<(&'a [usize], Vec<usize>), QueryError> {
        let split = selected.partition_point(|index| *index < values);
        self.budget
            .charge_allocation((selected.len() - split) as u64, size_of::<usize>() as u64)?;
        let mut named = Vec::new();
        named
            .try_reserve_exact(selected.len() - split)
            .map_err(|_| QueryError::Allocation)?;
        for index in &selected[split..] {
            named.push(index - values);
        }
        Ok((&selected[..split], named))
    }
    pub(super) fn multivalue_rows(
        &mut self,
        name: &str,
        columns: &[Column],
        projection: &[usize],
        multivalues: &[Column],
        members: &[CollectionMember],
        expansion: &[usize],
    ) -> Result<Vec<Vec<Value>>, QueryError> {
        let (values, named) = self.collection_requests(expansion, multivalues.len())?;
        let layout = self.collection_expansion(multivalues, members, expansion)?;
        let source = self.source;
        let mut rows = Vec::new();
        let mut failure: Option<QueryError> = None;
        let mut visitor = |scalars: ProjectedRow<'_>, lists: &[&dyn MultivalueList]| {
            if let Some(error) = &failure {
                return Err(error.clone());
            }
            let result = (|| {
                self.tick()?;
                if lists.len() != expansion.len() {
                    return Err(QueryError::Source);
                }
                let scalar = self.copy_projected_row(columns, projection, scalars)?;
                self.expand_collection_row(&scalar, &layout, lists, &mut rows)
            })();
            if let Err(error) = &result {
                failure = Some(error.clone());
            }
            result
        };
        let result = source.scan_collection_members(name, projection, values, &named, &mut visitor);
        failure.map_or(result.map(|()| rows), Err)
    }
}
