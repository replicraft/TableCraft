use super::{Environment, Runtime};
use crate::profile::PERCENT_TOTAL;
use crate::{
    QueryError,
    plan::{ReadNode, ReadPlan, SelectPlan},
};
use std::cmp::Ordering;
use tablecraft_core::values::Value;
use tablecraft_expressions::logic::Truth;

pub(super) struct OutputRow {
    pub(super) values: Vec<Value>,
    pub(super) keys: Vec<Value>,
}
struct Group {
    pub(super) keys: Vec<Value>,
    rows: Vec<Vec<Value>>,
}
impl Runtime<'_> {
    pub(super) fn read(
        &mut self,
        plan: &ReadPlan,
        outer: &[&[Value]],
    ) -> Result<Vec<Vec<Value>>, QueryError> {
        self.tick()?;
        let mut rows = match &plan.node {
            ReadNode::Select(select) => self.select(select, plan, outer)?,
            ReadNode::Union { first, rest } => {
                let mut rows = self.read(first, outer)?;
                for (all, arm) in rest {
                    for row in self.read(arm, outer)? {
                        self.push(&mut rows, row)?;
                    }
                    if !*all {
                        rows = self.unique(rows)?;
                    }
                }
                let mut output = Vec::new();
                for row in rows {
                    let env = Environment {
                        row: &row,
                        outer,
                        output: &row,
                        group: None,
                    };
                    let mut keys = self.allocate_row(plan.sort.len())?;
                    for (expr, _) in &plan.sort {
                        keys.push(self.eval(expr, env)?);
                    }
                    self.push(&mut output, OutputRow { values: row, keys })?;
                }
                output
            }
        };
        if plan.distinct {
            let mut unique: Vec<OutputRow> = Vec::new();
            for row in rows {
                let mut duplicate = false;
                for other in &unique {
                    if self.same_row(&row.values, &other.values)? {
                        duplicate = true;
                        break;
                    }
                }
                if !duplicate {
                    self.push(&mut unique, row)?;
                }
            }
            rows = unique;
        }
        rows = self.sort_rows(rows, plan)?;
        if let Some((count, percent)) = plan.top {
            let mut take = if percent {
                (rows.len() as u64 * u64::from(count)).div_ceil(u64::from(PERCENT_TOTAL)) as usize
            } else {
                count as usize
            };
            if take > 0 && !plan.sort.is_empty() {
                while take < rows.len()
                    && self.compare_keys(&rows[take - 1].keys, &rows[take].keys, plan)?
                        == Ordering::Equal
                {
                    take += 1;
                }
            }
            rows.truncate(take);
        }
        let mut result = Vec::new();
        for row in rows {
            self.push(&mut result, row.values)?;
        }
        Ok(result)
    }
    fn select(
        &mut self,
        select: &SelectPlan,
        plan: &ReadPlan,
        outer: &[&[Value]],
    ) -> Result<Vec<OutputRow>, QueryError> {
        let mut filtered = Vec::new();
        for row in self.relation(&select.source, outer)? {
            self.tick()?;
            if let Some(filter) = &select.filter {
                let value = self.eval(
                    filter,
                    Environment {
                        row: &row,
                        outer,
                        output: &[],
                        group: None,
                    },
                )?;
                if !Truth::from_value(&value)?.is_true() {
                    continue;
                }
            }
            self.push(&mut filtered, row)?;
        }
        let mut result = Vec::new();
        if select.aggregate {
            let groups = self.groups(filtered, select, outer)?;
            for group in groups {
                let env = Environment {
                    row: group.rows.first().map_or(&[], Vec::as_slice),
                    outer,
                    output: &[],
                    group: Some(&group.rows),
                };
                if let Some(row) = self.project(select, plan, env)? {
                    self.push(&mut result, row)?;
                }
            }
        } else {
            for row in filtered {
                if let Some(row) = self.project(
                    select,
                    plan,
                    Environment {
                        row: &row,
                        outer,
                        output: &[],
                        group: None,
                    },
                )? {
                    self.push(&mut result, row)?;
                }
            }
        }
        Ok(result)
    }
    fn project(
        &mut self,
        select: &SelectPlan,
        plan: &ReadPlan,
        env: Environment<'_>,
    ) -> Result<Option<OutputRow>, QueryError> {
        if let Some(having) = &select.having
            && !Truth::from_value(&self.eval(having, env)?)?.is_true()
        {
            return Ok(None);
        }
        let mut values = self.allocate_row(select.projection.len())?;
        for expr in &select.projection {
            values.push(self.eval(expr, env)?);
        }
        let mut keys = self.allocate_row(plan.sort.len())?;
        for (expr, _) in &plan.sort {
            keys.push(self.eval(
                expr,
                Environment {
                    output: &values,
                    ..env
                },
            )?);
        }
        Ok(Some(OutputRow { values, keys }))
    }
    fn groups(
        &mut self,
        rows: Vec<Vec<Value>>,
        select: &SelectPlan,
        outer: &[&[Value]],
    ) -> Result<Vec<Group>, QueryError> {
        if select.group.is_empty() {
            let mut groups = Vec::new();
            self.push(
                &mut groups,
                Group {
                    keys: Vec::new(),
                    rows,
                },
            )?;
            return Ok(groups);
        }
        let mut groups: Vec<Group> = Vec::new();
        for row in rows {
            let mut keys = self.allocate_row(select.group.len())?;
            for expr in &select.group {
                keys.push(self.eval(
                    expr,
                    Environment {
                        row: &row,
                        outer,
                        output: &[],
                        group: None,
                    },
                )?);
            }
            let mut index = None;
            for (i, group) in groups.iter().enumerate() {
                if self.same_row(&keys, &group.keys)? {
                    index = Some(i);
                    break;
                }
            }
            if let Some(i) = index {
                self.push(&mut groups[i].rows, row)?;
            } else {
                let mut members = Vec::new();
                self.push(&mut members, row)?;
                self.push(
                    &mut groups,
                    Group {
                        keys,
                        rows: members,
                    },
                )?;
            }
        }
        Ok(groups)
    }
    fn unique(&mut self, rows: Vec<Vec<Value>>) -> Result<Vec<Vec<Value>>, QueryError> {
        let mut result: Vec<Vec<Value>> = Vec::new();
        for row in rows {
            let mut duplicate = false;
            for other in &result {
                if self.same_row(&row, other)? {
                    duplicate = true;
                    break;
                }
            }
            if !duplicate {
                self.push(&mut result, row)?;
            }
        }
        Ok(result)
    }
    pub(super) fn compare_keys(
        &mut self,
        left: &[Value],
        right: &[Value],
        plan: &ReadPlan,
    ) -> Result<Ordering, QueryError> {
        self.tick()?;
        for ((left, right), (_, descending)) in left.iter().zip(right).zip(&plan.sort) {
            let order = self.order(left, right)?;
            if order != Ordering::Equal {
                return Ok(if *descending { order.reverse() } else { order });
            }
        }
        Ok(Ordering::Equal)
    }
}
