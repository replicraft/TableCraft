//! Fully loaded scalar pairs use the existing evaluator before list transport.
use super::{CollectionFrame, JoinedFrame, Requests, Runtime};
use crate::{
    QueryError,
    execution::{Environment, cell::CellValue},
    plan::{Expr, Relation, RelationNode},
    profile::MAX_RUNTIME_READ_DEPTH,
};
use tablecraft_sql::select::JoinKind;

struct JoinInput<'a> {
    left: &'a Relation,
    right: &'a Relation,
    condition: Option<&'a Expr>,
    kind: Option<JoinKind>,
}

impl Runtime<'_, '_> {
    pub(super) fn joined_collection_relation(
        &mut self,
        relation: &Relation,
        requests: &Requests<'_>,
        base: usize,
        leaf: &mut usize,
    ) -> Result<Vec<JoinedFrame>, QueryError> {
        self.tick()?;
        let previous_depth = self.discovery.active_read_depth.get();
        if previous_depth >= MAX_RUNTIME_READ_DEPTH {
            return Err(QueryError::StructureLimit);
        }
        self.discovery.active_read_depth.set(previous_depth + 1);
        let result = self.joined_collection_relation_owned(relation, requests, base, leaf);
        self.discovery.active_read_depth.set(previous_depth);
        result
    }

    fn joined_collection_relation_owned(
        &mut self,
        relation: &Relation,
        requests: &Requests<'_>,
        base: usize,
        leaf: &mut usize,
    ) -> Result<Vec<JoinedFrame>, QueryError> {
        match &relation.node {
            RelationNode::Cross(left, right) => self.joined_collection_pairs(
                JoinInput {
                    left,
                    right,
                    condition: None,
                    kind: None,
                },
                requests,
                base,
                leaf,
            ),
            RelationNode::Join {
                left,
                right,
                kind,
                condition,
            } => self.joined_collection_pairs(
                JoinInput {
                    left,
                    right,
                    condition: Some(condition),
                    kind: Some(*kind),
                },
                requests,
                base,
                leaf,
            ),
            RelationNode::Table { .. } => {
                let current = *leaf;
                *leaf = leaf.checked_add(1).ok_or(QueryError::StructureLimit)?;
                self.joined_collection_table(relation, requests, base, current)
            }
            RelationNode::Derived(read) => {
                *leaf = leaf.checked_add(1).ok_or(QueryError::StructureLimit)?;
                let mut result = Vec::new();
                if read.has_collections() {
                    for mut frame in self.collection_read(read)? {
                        for (index, _) in &mut frame.lists {
                            self.tick()?;
                            *index = index.checked_add(base).ok_or(QueryError::StructureLimit)?;
                        }
                        self.push(
                            &mut result,
                            JoinedFrame {
                                frame,
                                criteria: Vec::new(),
                            },
                        )?;
                    }
                } else {
                    for scalars in self.read(read, &[])? {
                        self.push(
                            &mut result,
                            JoinedFrame {
                                frame: CollectionFrame {
                                    scalars,
                                    lists: Vec::new(),
                                    keys: Vec::new(),
                                },
                                criteria: Vec::new(),
                            },
                        )?;
                    }
                }
                Ok(result)
            }
            RelationNode::Unit => Err(QueryError::Unsupported),
        }
    }

    fn joined_collection_pairs(
        &mut self,
        input: JoinInput<'_>,
        requests: &Requests<'_>,
        base: usize,
        leaf: &mut usize,
    ) -> Result<Vec<JoinedFrame>, QueryError> {
        // Both populations finish before ON or outer extension, even when the
        // opposite side is empty. A failed suffix never becomes a relation.
        let left_start = *leaf;
        let lhs = self.joined_collection_relation(input.left, requests, base, leaf)?;
        let right_start = *leaf;
        let right_base = base
            .checked_add(self.joined_collection_width(input.left)?)
            .ok_or(QueryError::StructureLimit)?;
        let rhs = self.joined_collection_relation(input.right, requests, right_base, leaf)?;
        let right_end = *leaf;
        let mut matched_right = Vec::new();
        if input.kind == Some(JoinKind::Right) {
            self.budget
                .charge_allocation(rhs.len() as u64, size_of::<bool>() as u64)?;
            matched_right
                .try_reserve_exact(rhs.len())
                .map_err(|_| QueryError::Allocation)?;
            matched_right.resize(rhs.len(), false);
        }
        let mut result = Vec::new();
        for left in &lhs {
            self.tick()?;
            let mut matched_left = false;
            for (position, right) in rhs.iter().enumerate() {
                self.tick()?;
                let scalars = self.join_row(&left.frame.scalars, &right.frame.scalars)?;
                if let Some(condition) = input.condition {
                    let accepted = self.eval_predicate(
                        condition,
                        Environment {
                            aggregate_results: None,
                            collections: None,
                            row: &scalars,
                            outer: &[],
                            output: &[],
                            group: None,
                            group_keys: None,
                        },
                    )?;
                    if !accepted.is_true() {
                        continue;
                    }
                }
                matched_left = true;
                if let Some(matched) = matched_right.get_mut(position) {
                    *matched = true;
                }
                let joined = self.copy_joined_collection_pair(left, right, scalars)?;
                self.push(&mut result, joined)?;
            }
            if !matched_left && input.kind == Some(JoinKind::Left) {
                let missing = self.missing_collection_side(
                    input.right,
                    requests,
                    right_base,
                    right_start..right_end,
                )?;
                let scalars =
                    self.join_missing(&left.frame.scalars, input.right.row_width(), false)?;
                let joined = self.copy_joined_collection_pair(left, &missing, scalars)?;
                self.push(&mut result, joined)?;
            }
        }
        if input.kind == Some(JoinKind::Right) {
            for (position, right) in rhs.iter().enumerate() {
                self.tick()?;
                if !matched_right[position] {
                    let missing = self.missing_collection_side(
                        input.left,
                        requests,
                        base,
                        left_start..right_start,
                    )?;
                    let scalars =
                        self.join_missing(&right.frame.scalars, input.left.row_width(), true)?;
                    let joined = self.copy_joined_collection_pair(&missing, right, scalars)?;
                    self.push(&mut result, joined)?;
                }
            }
        }
        Ok(result)
    }

    fn copy_joined_collection_pair(
        &mut self,
        left: &JoinedFrame,
        right: &JoinedFrame,
        scalars: Vec<CellValue>,
    ) -> Result<JoinedFrame, QueryError> {
        let mut lists = Vec::new();
        for (index, value) in left.frame.lists.iter().chain(&right.frame.lists) {
            self.tick()?;
            let copied = self.copy_collection_presence(value.as_ref())?;
            self.push(&mut lists, (*index, copied))?;
        }
        let mut criteria = Vec::new();
        for (index, value) in left.criteria.iter().chain(&right.criteria) {
            self.tick()?;
            let copied = self.copy_collection_presence(value.as_ref())?;
            self.push(&mut criteria, (*index, copied))?;
        }
        Ok(JoinedFrame {
            frame: CollectionFrame {
                scalars,
                lists,
                keys: Vec::new(),
            },
            criteria,
        })
    }
}
