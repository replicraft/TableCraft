//! Eager bounded products of aligned collection members within one atom.
use super::{ParentCollections, Runtime};
use crate::{QueryError, execution::Environment, plan::ParentProduct};
use tablecraft_core::values::ExpressionValue;
use tablecraft_expressions::logic::Truth;
use tablecraft_schema::validate_row;

impl Runtime<'_> {
    pub(super) fn parent_product(
        &mut self,
        product: &ParentProduct,
        collections: &ParentCollections<'_>,
        row: &[ExpressionValue],
        outer: &[&[ExpressionValue]],
    ) -> Result<Truth, QueryError> {
        self.tick()?;
        self.budget
            .charge_allocation(product.members.len() as u64, size_of::<usize>() as u64)?;
        self.budget.charge_allocation(
            product.groups as u64,
            (size_of::<Option<usize>>() + size_of::<usize>()) as u64,
        )?;
        let mut indices = Vec::new();
        let mut lengths = Vec::new();
        let mut positions = Vec::new();
        indices
            .try_reserve_exact(product.members.len())
            .map_err(|_| QueryError::Allocation)?;
        lengths
            .try_reserve_exact(product.groups)
            .map_err(|_| QueryError::Allocation)?;
        positions
            .try_reserve_exact(product.groups)
            .map_err(|_| QueryError::Allocation)?;
        lengths.resize(product.groups, None);
        positions.resize(product.groups, 0_usize);
        let mut absent = false;
        for member in &product.members {
            self.tick()?;
            let index = collections
                .requested
                .binary_search(&member.collection)
                .map_err(|_| QueryError::Source)?;
            let list = collections.lists.get(index).ok_or(QueryError::Source)?;
            let column = collections.columns.get(index).ok_or(QueryError::Source)?;
            let length = lengths.get_mut(member.group).ok_or(QueryError::Source)?;
            if length.is_some_and(|length| length != list.len()) {
                return Err(QueryError::Source);
            }
            *length = Some(list.len());
            absent |= collections.absent.is_some_and(|missing| missing[index]);
            // Validate every selected list even when another group is empty or
            // absent. No tuple expression runs without actual present items.
            for position in 0..list.len() {
                self.tick()?;
                let value = list.get(position).ok_or(QueryError::Source)?;
                validate_row(std::slice::from_ref(column), std::slice::from_ref(value))?;
            }
            indices.push(index);
        }
        if absent {
            return Ok(Truth::Unknown);
        }
        if lengths.contains(&Some(0)) {
            return Ok(Truth::False);
        }
        let mut truth = Truth::False;
        loop {
            self.tick()?;
            let mut values = self.allocate_expression_row(product.members.len())?;
            for (member, index) in product.members.iter().zip(&indices) {
                let value = collections.lists[*index]
                    .get(positions[member.group])
                    .ok_or(QueryError::Source)?;
                values.push(ExpressionValue::Value(self.copy(value)?));
            }
            let captured = if product.capture {
                let mut frame = self.allocate_expression_row(row.len() + values.len())?;
                for value in row.iter().chain(&values) {
                    frame.push(self.copy_expression(value)?);
                }
                Some(frame)
            } else {
                None
            };
            let value = self.eval(
                &product.predicate,
                Environment {
                    row: captured.as_deref().unwrap_or(row),
                    outer,
                    output: &values,
                    group: None,
                },
            )?;
            truth = truth.or(self.evaluator().predicate_truth(&value)?);
            let mut carry = true;
            for group in (0..product.groups).rev() {
                self.tick()?;
                positions[group] += 1;
                if positions[group] < lengths[group].ok_or(QueryError::Source)? {
                    carry = false;
                    break;
                }
                positions[group] = 0;
            }
            if carry {
                break;
            }
        }
        Ok(truth)
    }
}
