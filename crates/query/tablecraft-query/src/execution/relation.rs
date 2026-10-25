use super::{Environment, Runtime};
use crate::execution::cell::CellValue;
use crate::execution::context::OuterScope;
use crate::{
    QueryError,
    plan::{Relation, RelationNode},
    source::{ProjectedRow, ScanConstraint},
};
use tablecraft_core::values::Value;
use tablecraft_sql::select::JoinKind;

impl Runtime<'_, '_> {
    pub(super) fn relation(
        &mut self,
        relation: &Relation,
        outer: &[OuterScope<'_>],
    ) -> Result<Vec<Vec<CellValue>>, QueryError> {
        self.relation_constrained(relation, outer, None)
    }
    pub(super) fn relation_constrained(
        &mut self,
        relation: &Relation,
        outer: &[OuterScope<'_>],
        constraint: Option<ScanConstraint>,
    ) -> Result<Vec<Vec<CellValue>>, QueryError> {
        self.tick()?;
        self.check_table_schema(&relation.node)?;
        match &relation.node {
            RelationNode::Unit => {
                let row = self.allocate_expression_row(0)?;
                let mut rows = Vec::new();
                self.push(&mut rows, row)?;
                Ok(rows)
            }
            RelationNode::Derived(read) => self.read(read, &[]),
            RelationNode::Table {
                namespace,
                name,
                columns,
                projection,
                multivalues,
                members,
                expansion,
                ..
            } => {
                if !expansion.is_empty() {
                    let source = self.namespace_source(*namespace)?;
                    return self.multivalue_rows(
                        (source, name),
                        columns,
                        projection,
                        multivalues,
                        members,
                        expansion,
                    );
                }
                let mut rows = Vec::new();
                let source = self.namespace_source(*namespace)?;
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
    pub(super) fn check_table_schema(&mut self, node: &RelationNode) -> Result<(), QueryError> {
        let RelationNode::Table {
            namespace,
            name,
            columns,
            unavailable,
            text_collations,
            multivalue_text_collations,
            member_text_collations,
            multivalues,
            members,
            ..
        } = node
        else {
            return Ok(());
        };

        let source = self.namespace_source(*namespace)?;
        if source.columns(name)? != columns
            || source.unavailable_columns(name)? != unavailable
            || source.text_collations(name)? != text_collations
            || source.multivalue_text_collations(name)? != multivalue_text_collations
            || source.collection_member_text_collations(name)? != member_text_collations
            || source.multivalue_columns(name)? != multivalues
            || source.collection_members(name)? != members
        {
            return Err(QueryError::SchemaChanged);
        }

        Ok(())
    }

    fn join(
        &mut self,
        left: &Relation,
        right: &Relation,
        kind: Option<JoinKind>,
        condition: Option<&crate::plan::Expr>,
        outer: &[OuterScope<'_>],
    ) -> Result<Vec<Vec<CellValue>>, QueryError> {
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
                    self.eval_predicate(
                        e,
                        Environment {
                            counts: None,
                            collections: None,
                            row: &row,
                            outer,
                            output: &[],
                            group: None,
                            group_keys: None,
                        },
                    )?
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
    pub(in crate::execution) fn join_missing(
        &mut self,
        present: &[CellValue],
        missing: usize,
        missing_left: bool,
    ) -> Result<Vec<CellValue>, QueryError> {
        let width = present.len() + missing;
        let mut row = self.allocate_expression_row(width)?;
        if missing_left {
            row.resize_with(missing, || CellValue::scalar(Value::Null));
        }
        for value in present {
            row.push(self.copy_expression(value)?);
        }
        row.resize_with(width, || CellValue::scalar(Value::Null));
        Ok(row)
    }
    pub(in crate::execution) fn join_row(
        &mut self,
        left: &[CellValue],
        right: &[CellValue],
    ) -> Result<Vec<CellValue>, QueryError> {
        let mut row = self.allocate_expression_row(left.len() + right.len())?;
        for value in left.iter().chain(right) {
            row.push(self.copy_expression(value)?);
        }
        Ok(row)
    }
}
