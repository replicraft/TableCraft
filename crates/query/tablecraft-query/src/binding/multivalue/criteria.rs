//! Lift hidden member criteria to independent parent membership predicates.
use super::super::{Binder, projection::referenced_fields};
use crate::{
    QueryError,
    plan::{Expr, ParentFilter, ParentPredicate, Relation, RelationNode},
};
use tablecraft_sql::select::{BinaryOperator as B, UnaryOperator as U};

impl Binder<'_> {
    pub(in crate::binding) fn parent_criteria(
        &mut self,
        source: &Relation,
        projection: &[Expr],
        group: &[Expr],
        filter: Option<Expr>,
        depth: usize,
    ) -> Result<(Option<Expr>, Option<ParentFilter>), QueryError> {
        let Some(filter) = filter else {
            return Ok((None, None));
        };
        if !source.slots.iter().any(|slot| slot.member.is_some()) {
            return Ok((Some(filter), None));
        }
        let width = source.row_width();
        let used = referenced_fields(std::slice::from_ref(&filter), width, self)?;
        let mut shown = referenced_fields(projection, width, self)?;
        let grouped = referenced_fields(group, width, self)?;
        for (value, grouped) in shown.iter_mut().zip(grouped) {
            *value |= grouped;
        }
        let mut position = 0;
        let mut hidden = false;
        let mut displayed = false;
        for slot in &source.slots {
            if slot.available {
                if slot.member.is_some() {
                    hidden |= used[position] && !shown[position];
                    displayed |= shown[position];
                }
                position += 1;
            }
        }
        if !hidden {
            return Ok((Some(filter), None));
        }
        let RelationNode::Table {
            columns,
            multivalues,
            members,
            ..
        } = &source.node
        else {
            return Err(QueryError::Unsupported);
        };
        if displayed {
            return Err(QueryError::Unsupported);
        }
        let total = multivalues.len() + members.len();
        self.charge(total * (size_of::<usize>() + size_of::<tablecraft_schema::Column>()))?;
        let mut collections = Vec::new();
        collections
            .try_reserve_exact(total)
            .map_err(|_| QueryError::Allocation)?;
        collections.extend((0..total).filter(|index| used[columns.len() + index]));
        let mut selected_columns = Vec::new();
        selected_columns
            .try_reserve_exact(collections.len())
            .map_err(|_| QueryError::Allocation)?;
        for index in &collections {
            let column = if *index < multivalues.len() {
                &multivalues[*index]
            } else {
                members[*index - multivalues.len()].column()
            };
            self.charge(column.name().len())?;
            selected_columns.push(column.clone());
        }
        let predicate = self.parent_predicate(filter, width, columns.len(), depth + 1)?;
        Ok((
            None,
            Some(ParentFilter {
                collections,
                columns: selected_columns,
                predicate,
            }),
        ))
    }
    fn parent_predicate(
        &mut self,
        expr: Expr,
        width: usize,
        scalar_width: usize,
        depth: usize,
    ) -> Result<ParentPredicate, QueryError> {
        self.tick(depth)?;
        match expr {
            Expr::Unary(U::Not, value) => Ok(ParentPredicate::Not(Box::new(
                self.parent_predicate(*value, width, scalar_width, depth + 1)?,
            ))),
            Expr::Binary(
                op @ (B::And | B::Or | B::Xor | B::Equivalence | B::Implication),
                left,
                right,
            ) => Ok(ParentPredicate::Logical(
                op,
                Box::new(self.parent_predicate(*left, width, scalar_width, depth + 1)?),
                Box::new(self.parent_predicate(*right, width, scalar_width, depth + 1)?),
            )),
            mut expr => {
                if !self.same_group_expression(&expr, &expr, depth + 1)? {
                    return Err(QueryError::Unsupported);
                }
                let used = referenced_fields(std::slice::from_ref(&expr), width, self)?;
                let mut fields = (scalar_width..width).filter(|index| used[*index]);
                let Some(field) = fields.next() else {
                    return Ok(ParentPredicate::Scalar(expr));
                };
                if fields.next().is_some() {
                    return Err(QueryError::Unsupported);
                }
                let negated = match &mut expr {
                    Expr::Binary(operator @ B::NotEqual, _, _) => {
                        *operator = B::Equal;
                        true
                    }
                    Expr::Like(_, _, negated)
                    | Expr::Between(_, _, _, negated)
                    | Expr::InList(_, _, negated)
                    | Expr::IsNull(_, negated) => std::mem::take(negated),
                    _ => false,
                };
                self.replace_item(&mut expr, field, depth + 1)?;
                let predicate = ParentPredicate::Any {
                    collection: field - scalar_width,
                    predicate: expr,
                };
                Ok(if negated {
                    ParentPredicate::Not(Box::new(predicate))
                } else {
                    predicate
                })
            }
        }
    }
    fn replace_item(
        &mut self,
        expr: &mut Expr,
        field: usize,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        match expr {
            Expr::Field { depth: 0, index } if *index == field => *expr = Expr::Output(0),
            Expr::Unary(_, e) | Expr::IsNull(e, _) | Expr::DateCall(_, e) | Expr::Convert(_, e) => {
                self.replace_item(e, field, depth + 1)?
            }
            Expr::Binary(_, a, b) | Expr::Like(a, b, _) => {
                self.replace_item(a, field, depth + 1)?;
                self.replace_item(b, field, depth + 1)?;
            }
            Expr::Between(a, b, c, _) | Expr::IIf(a, b, c) | Expr::DateAdd(a, b, c) => {
                for e in [a, b, c] {
                    self.replace_item(e, field, depth + 1)?;
                }
            }
            Expr::DateDiff(a, b, c, options) => {
                for e in [a, b, c].into_iter().chain(options.iter_mut().flatten()) {
                    self.replace_item(e, field, depth + 1)?;
                }
            }
            Expr::DatePart(a, b, options) => {
                for e in [a, b].into_iter().chain(options.iter_mut().flatten()) {
                    self.replace_item(e, field, depth + 1)?;
                }
            }
            Expr::TextCall(_, args)
            | Expr::MathCall(_, args)
            | Expr::ConstructorCall(_, args)
            | Expr::ConditionalCall(_, args) => {
                for e in args {
                    self.replace_item(e, field, depth + 1)?;
                }
            }
            Expr::Nz(value, replacement) => {
                self.replace_item(value, field, depth + 1)?;
                if let Some(e) = replacement {
                    self.replace_item(e, field, depth + 1)?;
                }
            }
            Expr::InList(value, list, _) => {
                self.replace_item(value, field, depth + 1)?;
                for e in list {
                    self.replace_item(e, field, depth + 1)?;
                }
            }
            Expr::Literal(_) | Expr::Field { .. } | Expr::Parameter(_) | Expr::Output(_) => {}
            Expr::Aggregate(..)
            | Expr::Subquery(_)
            | Expr::Exists(_)
            | Expr::InQuery(..)
            | Expr::Quantified(..) => return Err(QueryError::Unsupported),
        }
        Ok(())
    }
}
