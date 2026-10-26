//! Fully loaded scalar pairs use the existing evaluator before list transport.
use super::{CollectionFrame, JoinedFrame, Requests, Runtime};
use crate::{
    QueryError,
    execution::Environment,
    plan::{Expr, Relation, RelationNode},
    profile::MAX_RUNTIME_READ_DEPTH,
};
use tablecraft_sql::select::JoinKind;

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
            RelationNode::Cross(left, right) => {
                self.joined_collection_pairs(left, right, None, requests, base, leaf)
            }
            RelationNode::Join {
                left,
                right,
                kind: JoinKind::Inner,
                condition,
            } => self.joined_collection_pairs(left, right, Some(condition), requests, base, leaf),
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
            RelationNode::Unit | RelationNode::Join { .. } => Err(QueryError::Unsupported),
        }
    }

    fn joined_collection_pairs(
        &mut self,
        left: &Relation,
        right: &Relation,
        condition: Option<&Expr>,
        requests: &Requests<'_>,
        base: usize,
        leaf: &mut usize,
    ) -> Result<Vec<JoinedFrame>, QueryError> {
        // The complete right population is still read and validated when the
        // left is empty. Neither WHERE nor TOP can hide input or ON failures.
        let lhs = self.joined_collection_relation(left, requests, base, leaf)?;
        let right_base = base
            .checked_add(self.joined_collection_width(left)?)
            .ok_or(QueryError::StructureLimit)?;
        let rhs = self.joined_collection_relation(right, requests, right_base, leaf)?;
        let mut result = Vec::new();
        for left in &lhs {
            self.tick()?;
            for right in &rhs {
                self.tick()?;
                let scalars = self.join_row(&left.frame.scalars, &right.frame.scalars)?;
                if let Some(condition) = condition {
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
                let mut lists = Vec::new();
                for (index, value) in left.frame.lists.iter().chain(&right.frame.lists) {
                    self.tick()?;
                    let copied = self.copy_collection_values(value.values())?;
                    self.push(&mut lists, (*index, copied))?;
                }
                let mut criteria = Vec::new();
                for (index, value) in left.criteria.iter().chain(&right.criteria) {
                    self.tick()?;
                    let copied = self.copy_collection_values(value.values())?;
                    self.push(&mut criteria, (*index, copied))?;
                }
                self.push(
                    &mut result,
                    JoinedFrame {
                        frame: CollectionFrame {
                            scalars,
                            lists,
                            keys: Vec::new(),
                        },
                        criteria,
                    },
                )?;
            }
        }
        Ok(result)
    }
}
