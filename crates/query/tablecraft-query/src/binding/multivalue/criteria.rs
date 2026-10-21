//! Lift hidden member criteria to independent parent membership predicates.
use super::super::{
    Binder,
    projection::{referenced_fields, relation_fields},
};
use super::displayed::Displayed;
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
        let mut used = referenced_fields(std::slice::from_ref(&filter), width, self)?;
        let mut shown = referenced_fields(projection, width, self)?;
        let grouped = referenced_fields(group, width, self)?;
        let joined = relation_fields(source, self)?;
        for ((value, grouped), joined) in shown.iter_mut().zip(grouped).zip(joined) {
            *value |= grouped || joined;
        }
        let displayed_collections = Displayed::new(source, &shown, self)?;
        let mut position = 0;
        let mut hidden = false;
        let mut displayed = false;
        for slot in &source.slots {
            if slot.available {
                if slot.member.is_some() {
                    used[position] &= !displayed_collections.contains(slot);
                    hidden |= used[position];
                    displayed |= shown[position];
                }
                position += 1;
            }
        }
        if !hidden {
            return Ok((Some(filter), None));
        }
        if displayed {
            return Ok((
                None,
                Some(self.joined_parent(source, filter, &used, depth + 1)?),
            ));
        }
        let RelationNode::Table {
            columns,
            multivalues,
            members,
            ..
        } = &source.node
        else {
            return Ok((
                None,
                Some(self.joined_parent(source, filter, &used, depth + 1)?),
            ));
        };
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
        self.charge(collections.len() * size_of::<(usize, usize)>())?;
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(collections.len())
            .map_err(|_| QueryError::Allocation)?;
        fields.extend(
            collections
                .iter()
                .map(|index| (columns.len() + index, *index)),
        );
        let predicate = self.parent_predicate(filter, width, &fields, depth + 1)?;
        Ok((
            None,
            Some(ParentFilter {
                collections,
                columns: selected_columns,
                predicate,
                locations: None,
            }),
        ))
    }
    pub(super) fn parent_predicate(
        &mut self,
        expr: Expr,
        width: usize,
        virtual_fields: &[(usize, usize)],
        depth: usize,
    ) -> Result<ParentPredicate, QueryError> {
        self.tick(depth)?;
        match expr {
            Expr::Unary(U::Not, value) => Ok(ParentPredicate::Not(Box::new(
                self.parent_predicate(*value, width, virtual_fields, depth + 1)?,
            ))),
            Expr::Binary(
                op @ (B::And | B::Or | B::Xor | B::Equivalence | B::Implication),
                left,
                right,
            ) => Ok(ParentPredicate::Logical(
                op,
                Box::new(self.parent_predicate(*left, width, virtual_fields, depth + 1)?),
                Box::new(self.parent_predicate(*right, width, virtual_fields, depth + 1)?),
            )),
            mut expr => {
                let used = referenced_fields(std::slice::from_ref(&expr), width, self)?;
                let mut fields = virtual_fields.iter().filter(|(field, _)| used[*field]);
                let Some(&(field, collection)) = fields.next() else {
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
                    | Expr::InQuery(_, _, negated)
                    | Expr::IsNull(_, negated) => std::mem::take(negated),
                    _ => false,
                };
                let mut capture = false;
                self.replace_item(&mut expr, field, 0, &mut capture, depth + 1)?;
                let predicate = ParentPredicate::Any {
                    collection,
                    predicate: expr,
                    capture,
                };
                Ok(if negated {
                    ParentPredicate::Not(Box::new(predicate))
                } else {
                    predicate
                })
            }
        }
    }
}
