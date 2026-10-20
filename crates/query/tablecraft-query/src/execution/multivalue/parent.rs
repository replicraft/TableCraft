use super::Runtime;
use crate::{
    QueryError,
    execution::Environment,
    plan::{ParentFilter, ParentPredicate, Relation, RelationNode},
    source::{MultivalueList, MultivalueVisitor},
};
use tablecraft_core::values::Value;
use tablecraft_expressions::logic::Truth;
use tablecraft_schema::{Column, validate_row};
use tablecraft_sql::select::BinaryOperator as B;

impl Runtime<'_> {
    pub(in crate::execution) fn parent_rows(
        &mut self,
        relation: &Relation,
        parent: &ParentFilter,
        outer: &[&[Value]],
    ) -> Result<Vec<Vec<Value>>, QueryError> {
        let RelationNode::Table {
            name,
            columns,
            unavailable,
            projection,
            multivalues,
            expansion,
        } = &relation.node
        else {
            return Err(QueryError::Unsupported);
        };
        if !expansion.is_empty() {
            return Err(QueryError::Unsupported);
        }
        if self.source.columns(name)? != columns
            || self.source.unavailable_columns(name)? != unavailable
            || self.source.multivalue_columns(name)? != multivalues
        {
            return Err(QueryError::SchemaChanged);
        }
        let source = self.source;
        let mut rows = Vec::new();
        let mut failure: Option<QueryError> = None;
        let mut visit = |scalars: crate::source::ProjectedRow<'_>,
                         lists: &[&dyn MultivalueList]| {
            if let Some(error) = &failure {
                return Err(error.clone());
            }
            let result = (|| {
                self.tick()?;
                if lists.len() != parent.collections.len() {
                    return Err(QueryError::Source);
                }
                let row = self.copy_projected_row(columns, projection, scalars)?;
                let truth = self.parent_truth(
                    &parent.predicate,
                    &parent.collections,
                    multivalues,
                    lists,
                    &row,
                    outer,
                )?;
                if truth.is_true() {
                    self.push(&mut rows, row)?;
                }
                Ok(())
            })();
            if let Err(error) = &result {
                failure = Some(error.clone());
            }
            result
        };
        let visitor: &mut MultivalueVisitor<'_> = &mut visit;
        let result = source.scan_multivalues(name, projection, &parent.collections, visitor);
        failure.map_or(result.map(|()| rows), Err)
    }
    fn parent_truth(
        &mut self,
        predicate: &ParentPredicate,
        requested: &[usize],
        columns: &[Column],
        lists: &[&dyn MultivalueList],
        row: &[Value],
        outer: &[&[Value]],
    ) -> Result<Truth, QueryError> {
        self.tick()?;
        let env = Environment {
            row,
            outer,
            output: &[],
            group: None,
        };
        Ok(match predicate {
            ParentPredicate::Scalar(expr) => Truth::from_value(&self.eval(expr, env)?)?,
            ParentPredicate::Any {
                collection,
                predicate,
            } => {
                let position = requested
                    .binary_search(collection)
                    .map_err(|_| QueryError::Source)?;
                let list = lists.get(position).ok_or(QueryError::Source)?;
                let column = columns.get(*collection).ok_or(QueryError::Source)?;
                let mut truth = Truth::False;
                for index in 0..list.len() {
                    self.tick()?;
                    let value = list.get(index).ok_or(QueryError::Source)?;
                    validate_row(std::slice::from_ref(column), std::slice::from_ref(value))?;
                    let value = self.copy(value)?;
                    let result = self.eval(
                        predicate,
                        Environment {
                            output: std::slice::from_ref(&value),
                            ..env
                        },
                    )?;
                    truth = truth.or(Truth::from_value(&result)?);
                }
                truth
            }
            ParentPredicate::Not(value) => self
                .parent_truth(value, requested, columns, lists, row, outer)?
                .negate(),
            ParentPredicate::Logical(op, a, b) => {
                let a = self.parent_truth(a, requested, columns, lists, row, outer)?;
                let b = self.parent_truth(b, requested, columns, lists, row, outer)?;
                match op {
                    B::And => a.and(b),
                    B::Or => a.or(b),
                    B::Xor => a.xor(b),
                    B::Equivalence => a.equivalent(b),
                    B::Implication => a.implies(b),
                    _ => return Err(QueryError::Unsupported),
                }
            }
        })
    }
}
