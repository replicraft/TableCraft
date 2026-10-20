use super::{Environment, Runtime};
use crate::{
    QueryError,
    plan::{Relation, RelationNode},
    source::{ProjectedRow, ScanConstraint},
};
use tablecraft_core::values::Value;
use tablecraft_expressions::logic::Truth;
use tablecraft_sql::select::JoinKind;

impl Runtime<'_> {
    pub(super) fn relation(
        &mut self,
        relation: &Relation,
        outer: &[&[Value]],
    ) -> Result<Vec<Vec<Value>>, QueryError> {
        self.relation_constrained(relation, outer, None)
    }
    pub(super) fn relation_constrained(
        &mut self,
        relation: &Relation,
        outer: &[&[Value]],
        constraint: Option<ScanConstraint>,
    ) -> Result<Vec<Vec<Value>>, QueryError> {
        self.tick()?;
        match &relation.node {
            RelationNode::Unit => {
                let row = self.allocate_row(0)?;
                let mut rows = Vec::new();
                self.push(&mut rows, row)?;
                Ok(rows)
            }
            RelationNode::Derived(read) => self.read(read, &[]),
            RelationNode::Table {
                name,
                columns,
                unavailable,
                projection,
                multivalues,
                members,
                expansion,
            } => {
                if self.source.columns(name)? != columns
                    || self.source.unavailable_columns(name)? != unavailable
                    || self.source.multivalue_columns(name)? != multivalues
                    || self.source.collection_members(name)? != members
                {
                    return Err(QueryError::SchemaChanged);
                }
                if !expansion.is_empty() {
                    return self.multivalue_rows(
                        name,
                        columns,
                        projection,
                        multivalues,
                        members,
                        expansion,
                    );
                }
                let mut rows = Vec::new();
                let source = self.source;
                let mut failure: Option<QueryError> = None;
                let mut visitor = |row: ProjectedRow<'_>| {
                    if let Some(error) = &failure {
                        return Err(error.clone());
                    }
                    let result = (|| {
                        self.tick()?;
                        let owned = self.copy_projected_row(columns, projection, row)?;
                        self.push(&mut rows, owned)
                    })();
                    if let Err(error) = &result {
                        failure = Some(error.clone());
                    }
                    result
                };
                let scan = source.scan_projected(name, projection, constraint, &mut visitor);
                if let Some(error) = failure {
                    return Err(error);
                }
                scan?;
                Ok(rows)
            }
            RelationNode::Cross(left, right) => self.join(left, right, None, None, outer),
            RelationNode::Join {
                left,
                right,
                kind,
                condition,
            } => self.join(left, right, Some(*kind), Some(condition), outer),
        }
    }
    fn join(
        &mut self,
        left: &Relation,
        right: &Relation,
        kind: Option<JoinKind>,
        condition: Option<&crate::plan::Expr>,
        outer: &[&[Value]],
    ) -> Result<Vec<Vec<Value>>, QueryError> {
        let lhs = self.relation(left, outer)?;
        let rhs = self.relation(right, outer)?;
        let mut result = Vec::new();
        // Matched flags are bounded by the materialized source and charged.
        self.budget
            .charge(tablecraft_core::limits::Resource::Bytes, rhs.len() as u64)?;
        let mut matched = Vec::new();
        matched
            .try_reserve_exact(rhs.len())
            .map_err(|_| QueryError::Allocation)?;
        matched.resize(rhs.len(), false);
        for l in &lhs {
            let mut hit = false;
            for (j, r) in rhs.iter().enumerate() {
                let row = self.join_row(l, r)?;
                let accepted = if let Some(e) = condition {
                    Truth::from_value(&self.eval(
                        e,
                        Environment {
                            row: &row,
                            outer,
                            output: &[],
                            group: None,
                        },
                    )?)?
                    .is_true()
                } else {
                    true
                };
                if accepted {
                    hit = true;
                    matched[j] = true;
                    self.push(&mut result, row)?;
                }
            }
            if !hit && kind == Some(JoinKind::Left) {
                let row = self.join_missing(l, right.row_width(), false)?;
                self.push(&mut result, row)?;
            }
        }
        if kind == Some(JoinKind::Right) {
            for (j, r) in rhs.iter().enumerate() {
                if !matched[j] {
                    let row = self.join_missing(r, left.row_width(), true)?;
                    self.push(&mut result, row)?;
                }
            }
        }
        Ok(result)
    }
    fn join_missing(
        &mut self,
        present: &[Value],
        missing: usize,
        missing_left: bool,
    ) -> Result<Vec<Value>, QueryError> {
        let width = present.len() + missing;
        let mut row = self.allocate_row(width)?;
        if missing_left {
            row.resize(missing, Value::Null);
        }
        for value in present {
            row.push(self.copy(value)?);
        }
        row.resize(width, Value::Null);
        Ok(row)
    }
    fn join_row(&mut self, left: &[Value], right: &[Value]) -> Result<Vec<Value>, QueryError> {
        let mut row = self.allocate_row(left.len() + right.len())?;
        for value in left.iter().chain(right) {
            row.push(self.copy(value)?);
        }
        Ok(row)
    }
}
