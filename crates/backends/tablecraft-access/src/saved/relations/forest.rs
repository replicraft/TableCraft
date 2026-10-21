//! Arena construction retains endpoint orientation and innermost ON placement.
use super::{Context, E, Source, conditions::Condition, kind};
use crate::saved::fragments::{Fragments, expression};
use tablecraft_core::limits::Resource;
use tablecraft_sql::select::JoinKind;

type Members = u16;
pub(super) struct Node<'a> {
    pub members: Members,
    pub body: Body<'a>,
}
pub(super) enum Body<'a> {
    Leaf(usize),
    Join {
        left: usize,
        right: usize,
        kind: JoinKind,
        conditions: Vec<Condition<'a>>,
    },
}
pub(super) struct Forest<'a> {
    pub sources: Vec<Source<'a>>,
    pub nodes: Vec<Node<'a>>,
    pub roots: Vec<usize>,
}
impl<'a> Forest<'a> {
    pub fn build(fragments: &Fragments<'a>, context: &mut Context<'_>) -> Result<Self, E> {
        let count = fragments.table_count;
        let mut forest = Self {
            sources: context.reserve(count)?,
            nodes: context.reserve(count * 2 - 1)?,
            roots: context.reserve(count)?,
        };
        for row in fragments.tables.iter().take(count) {
            let source = Source::read(row.ok_or(E::InvalidMetadata)?, context)?;
            if count > 1 && source.label().is_some_and(|label| !label.is_ascii()) {
                return Err(E::Unsupported);
            }
            for before in &forest.sources {
                context.check()?;
                let (Some(before), Some(label)) = (before.label(), source.label()) else {
                    continue;
                };
                context
                    .budget
                    .charge(Resource::Steps, (before.len() + label.len()) as u64)?;
                if before.eq_ignore_ascii_case(label) {
                    return Err(E::InvalidMetadata);
                }
            }
            let index = forest.sources.len();
            forest.sources.push(source);
            forest.nodes.push(Node {
                members: 1 << index,
                body: Body::Leaf(index),
            });
            forest.roots.push(index);
        }
        for row in fragments.joins.iter().take(fragments.join_count) {
            context.check()?;
            let row = row.ok_or(E::InvalidMetadata)?;
            let kind = kind(row)?;
            let left = forest.endpoint(row.name1(), context)?;
            let right = forest.endpoint(row.name2(), context)?;
            if left == right {
                return Err(E::InvalidMetadata);
            }
            let both = (1 << left) | (1 << right);
            let mut left_root = None;
            let mut right_root = None;
            for &root in &forest.roots {
                context.check()?;
                if forest.nodes[root].members & (1 << left) != 0 {
                    left_root = Some(root);
                }
                if forest.nodes[root].members & (1 << right) != 0 {
                    right_root = Some(root);
                }
            }
            let left_root = left_root.ok_or(E::InvalidMetadata)?;
            let right_root = right_root.ok_or(E::InvalidMetadata)?;
            let text = expression(row)?;
            let parsed = context.expression(text)?;
            let condition = Condition { text, parsed };
            if left_root == right_root {
                let target = forest.target(left_root, both, kind, context)?;
                let Body::Join { conditions, .. } = &mut forest.nodes[target].body else {
                    return Err(E::InvalidMetadata);
                };
                context
                    .budget
                    .charge_allocation(1, size_of::<Condition<'_>>() as u64)?;
                conditions.try_reserve(1).map_err(|_| E::Allocation)?;
                conditions.push(condition);
            } else {
                let mut conditions = context.reserve(1)?;
                conditions.push(condition);
                let members = forest.nodes[left_root].members | forest.nodes[right_root].members;
                let index = forest.nodes.len();
                forest.nodes.push(Node {
                    members,
                    body: Body::Join {
                        left: left_root,
                        right: right_root,
                        kind,
                        conditions,
                    },
                });
                context
                    .budget
                    .charge(Resource::Steps, forest.roots.len() as u64)?;
                forest
                    .roots
                    .retain(|&root| root != left_root && root != right_root);
                forest.roots.push(index);
            }
        }
        Ok(forest)
    }
    fn endpoint(&self, name: Option<&str>, context: &mut Context<'_>) -> Result<usize, E> {
        let name = name.ok_or(E::InvalidMetadata)?;
        if !name.is_ascii() {
            return Err(E::Unsupported);
        }
        for (index, source) in self.sources.iter().enumerate() {
            context.check()?;
            let Some(label) = source.label() else {
                continue;
            };
            context
                .budget
                .charge(Resource::Steps, (name.len() + label.len()) as u64)?;
            if label.eq_ignore_ascii_case(name) {
                return Ok(index);
            }
        }
        Err(E::InvalidMetadata)
    }
    fn target(
        &self,
        mut index: usize,
        members: Members,
        kind: JoinKind,
        context: &mut Context<'_>,
    ) -> Result<usize, E> {
        let mut selected = None;
        loop {
            context.check()?;
            let Body::Join {
                left,
                right,
                kind: actual,
                ..
            } = &self.nodes[index].body
            else {
                break;
            };
            if *actual == kind {
                selected = Some(index);
            }
            if self.nodes[*left].members & members == members {
                index = *left;
            } else if self.nodes[*right].members & members == members {
                index = *right;
            } else {
                break;
            }
        }
        selected.ok_or(E::Unsupported)
    }
}
