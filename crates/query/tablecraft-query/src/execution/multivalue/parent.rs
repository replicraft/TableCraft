use super::Runtime;
use crate::{
    QueryError,
    execution::Environment,
    plan::{ParentFilter, ParentPredicate, Relation, RelationNode},
    source::{MultivalueList, MultivalueVisitor},
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::logic::Truth;
use tablecraft_schema::{Column, validate_row};
use tablecraft_sql::select::BinaryOperator as B;
mod joined;
mod product;

struct ParentCollections<'a> {
    requested: &'a [usize],
    columns: &'a [Column],
    lists: &'a [&'a dyn MultivalueList],
    absent: Option<&'a [bool]>,
}

impl Runtime<'_> {
    pub(in crate::execution) fn parent_rows(
        &mut self,
        relation: &Relation,
        parent: &ParentFilter,
        outer: &[&[ExpressionValue]],
    ) -> Result<Vec<Vec<ExpressionValue>>, QueryError> {
        self.parent_rows_before_filter(relation, parent, outer, &mut |_, _| Ok(None))
    }
    // A crosstab discovers its pivot before membership and carries that value
    // in an appended private cell. Ordinary retrieval adds no such cell.
    pub(in crate::execution) fn parent_rows_before_filter<F>(
        &mut self,
        relation: &Relation,
        parent: &ParentFilter,
        outer: &[&[ExpressionValue]],
        before: &mut F,
    ) -> Result<Vec<Vec<ExpressionValue>>, QueryError>
    where
        F: FnMut(&mut Self, &[ExpressionValue]) -> Result<Option<Value>, QueryError>,
    {
        if parent.locations.is_some() {
            return self.joined_parent_rows(relation, parent, outer, before);
        }
        let RelationNode::Table {
            name,
            columns,
            unavailable,
            text_collations,
            multivalue_text_collations,
            member_text_collations,
            projection,
            multivalues,
            members,
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
            || self.source.text_collations(name)? != text_collations
            || self.source.multivalue_text_collations(name)? != multivalue_text_collations
            || self.source.collection_member_text_collations(name)? != member_text_collations
            || self.source.multivalue_columns(name)? != multivalues
            || self.source.collection_members(name)? != members
        {
            return Err(QueryError::SchemaChanged);
        }
        let source = self.source;
        let (values, named) = self.collection_requests(&parent.collections, multivalues.len())?;
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
                for (position, index) in parent.collections.iter().enumerate() {
                    if let Some(member_index) = index.checked_sub(multivalues.len()) {
                        let member = members.get(member_index).ok_or(QueryError::Source)?;
                        for (other_position, other_index) in
                            parent.collections[..position].iter().enumerate()
                        {
                            self.tick()?;
                            if let Some(other_member_index) =
                                other_index.checked_sub(multivalues.len())
                            {
                                let other =
                                    members.get(other_member_index).ok_or(QueryError::Source)?;
                                if crate::profile::SOURCE_SCHEMA_POLICY.names.equals(
                                    member.collection().as_str(),
                                    other.collection().as_str(),
                                ) && lists[position].len() != lists[other_position].len()
                                {
                                    return Err(QueryError::Source);
                                }
                            }
                        }
                    }
                }
                let mut row = self.copy_projected_row(columns, projection, scalars)?;
                if let Some(value) = before(self, &row)? {
                    self.push(&mut row, ExpressionValue::Value(value))?;
                }
                let truth = self.parent_truth(
                    &parent.predicate,
                    &ParentCollections {
                        requested: &parent.collections,
                        columns: &parent.columns,
                        lists,
                        absent: None,
                    },
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
        let result = source.scan_collection_members(name, projection, values, &named, visitor);
        failure.map_or(result.map(|()| rows), Err)
    }
    fn parent_truth(
        &mut self,
        predicate: &ParentPredicate,
        collections: &ParentCollections<'_>,
        row: &[ExpressionValue],
        outer: &[&[ExpressionValue]],
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
                capture,
            } => {
                let position = collections
                    .requested
                    .binary_search(collection)
                    .map_err(|_| QueryError::Source)?;
                if collections.absent.is_some_and(|missing| missing[position]) {
                    return Ok(Truth::Unknown);
                }
                let list = collections.lists.get(position).ok_or(QueryError::Source)?;
                let column = collections
                    .columns
                    .get(position)
                    .ok_or(QueryError::Source)?;
                let mut truth = Truth::False;
                for index in 0..list.len() {
                    self.tick()?;
                    let value = list.get(index).ok_or(QueryError::Source)?;
                    validate_row(std::slice::from_ref(column), std::slice::from_ref(value))?;
                    let value = ExpressionValue::Value(self.copy(value)?);
                    // Only correlated item references need an owned scope
                    // frame. Ordinary membership keeps its borrowed row path.
                    let item_row = if *capture {
                        let mut frame = self.allocate_expression_row(row.len() + 1)?;
                        for scalar in row {
                            frame.push(self.copy_expression(scalar)?);
                        }
                        frame.push(self.copy_expression(&value)?);
                        Some(frame)
                    } else {
                        None
                    };
                    let result = self.eval(
                        predicate,
                        Environment {
                            row: item_row.as_deref().unwrap_or(row),
                            output: std::slice::from_ref(&value),
                            ..env
                        },
                    )?;
                    truth = truth.or(Truth::from_value(&result)?);
                }
                truth
            }
            ParentPredicate::AnyMany(product) => {
                self.parent_product(product, collections, row, outer)?
            }
            ParentPredicate::Not(value) => {
                self.parent_truth(value, collections, row, outer)?.negate()
            }
            ParentPredicate::Logical(op, a, b) => {
                let a = self.parent_truth(a, collections, row, outer)?;
                let b = self.parent_truth(b, collections, row, outer)?;
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
