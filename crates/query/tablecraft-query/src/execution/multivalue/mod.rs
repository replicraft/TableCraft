use super::Runtime;
use crate::{
    QueryError,
    source::{MultivalueList, ProjectedRow},
};
use tablecraft_core::values::Value;
use tablecraft_schema::{Column, validate_row};

impl Runtime<'_> {
    pub(super) fn multivalue_rows(
        &mut self,
        name: &str,
        columns: &[Column],
        projection: &[usize],
        multivalues: &[Column],
        expansion: &[usize],
    ) -> Result<Vec<Vec<Value>>, QueryError> {
        let mut rows = Vec::new();
        let source = self.source;
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
                let mut expanded = Vec::new();
                let scalar = self.copy_projected_row(columns, projection, scalars)?;
                self.push(&mut expanded, scalar)?;
                for (list, selected) in lists.iter().zip(expansion) {
                    let column = multivalues.get(*selected).ok_or(QueryError::Source)?;
                    let length = list.len();
                    let mut next = Vec::new();
                    for row in expanded {
                        for index in 0..length.max(1) {
                            self.tick()?;
                            let mut output = self.allocate_row(row.len() + 1)?;
                            for value in &row {
                                output.push(self.copy(value)?);
                            }
                            let value = if length == 0 {
                                &Value::Null
                            } else {
                                let value = list.get(index).ok_or(QueryError::Source)?;
                                validate_row(
                                    std::slice::from_ref(column),
                                    std::slice::from_ref(value),
                                )?;
                                value
                            };
                            output.push(self.copy(value)?);
                            self.push(&mut next, output)?;
                        }
                    }
                    expanded = next;
                }
                for row in expanded {
                    self.push(&mut rows, row)?;
                }
                Ok(())
            })();
            if let Err(error) = &result {
                failure = Some(error.clone());
            }
            result
        };
        let result = source.scan_multivalues(name, projection, expansion, &mut visitor);
        failure.map_or(result.map(|()| rows), Err)
    }
}
