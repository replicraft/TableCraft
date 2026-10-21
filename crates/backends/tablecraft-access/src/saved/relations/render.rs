//! Parse a bounded forest carrier and verify every source and ON expression.
use super::{
    Context, E, conditions,
    forest::{Body, Forest},
    source::Body as SourceBody,
};
use tablecraft_sql::select::{JoinKind, TableSource};

const PARTS_PER_NODE: usize = 16;
const PARTS_PER_CONDITION: usize = 4;

pub(super) fn read(forest: &Forest<'_>, context: &mut Context<'_>) -> Result<Vec<TableSource>, E> {
    let conditions = forest
        .nodes
        .iter()
        .map(|node| match &node.body {
            Body::Join { conditions, .. } => conditions.len(),
            _ => 0,
        })
        .sum::<usize>();
    let mut parts =
        context.reserve(forest.nodes.len() * PARTS_PER_NODE + conditions * PARTS_PER_CONDITION)?;
    parts.push("SELECT 0 FROM ");
    for (index, &root) in forest.roots.iter().enumerate() {
        context.check()?;
        if index != 0 {
            parts.push(", ");
        }
        render(forest, root, true, &mut parts, context)?;
    }
    let mut parsed = context.select(&parts)?;
    if parsed.selection.is_some() || parsed.from.len() != forest.roots.len() {
        return Err(E::InvalidMetadata);
    }
    for (&root, actual) in forest.roots.iter().zip(&parsed.from) {
        verify(forest, root, actual, context)?;
    }
    context.check()?;
    Ok(std::mem::take(&mut parsed.from))
}
fn render<'a>(
    forest: &'a Forest<'a>,
    index: usize,
    root: bool,
    parts: &mut Vec<&'a str>,
    context: &mut Context<'_>,
) -> Result<(), E> {
    context.check()?;
    match &forest.nodes[index].body {
        Body::Leaf(source) => {
            let source = &forest.sources[*source];
            match &source.body {
                SourceBody::Named(name) => parts.extend(["[", name, "]"]),
                SourceBody::Derived { text, .. } => parts.extend(["(", text, "\n)"]),
            }
            if let Some(alias) = source.alias {
                parts.extend([" AS [", alias, "]"]);
            }
        }
        Body::Join {
            left,
            right,
            kind,
            conditions,
        } => {
            if !root {
                parts.push("(");
            }
            render(forest, *left, false, parts, context)?;
            parts.push(match kind {
                JoinKind::Inner => " INNER JOIN ",
                JoinKind::Left => " LEFT JOIN ",
                JoinKind::Right => " RIGHT JOIN ",
            });
            render(forest, *right, false, parts, context)?;
            parts.push(" ON (");
            for (position, condition) in conditions.iter().rev().enumerate() {
                context.check()?;
                if position != 0 {
                    parts.push(" AND ");
                }
                parts.extend(["(", condition.text, "\n)"]);
            }
            parts.push(")");
            if !root {
                parts.push(")");
            }
        }
    }
    Ok(())
}
fn verify(
    forest: &Forest<'_>,
    index: usize,
    actual: &TableSource,
    context: &mut Context<'_>,
) -> Result<(), E> {
    context.check()?;
    match &forest.nodes[index].body {
        Body::Leaf(source) => {
            let expected = &forest.sources[*source];
            let matches = match (&expected.body, actual) {
                (SourceBody::Named(name), TableSource::Table(table)) => {
                    table.name.as_str() == *name
                        && table.alias.as_ref().map(|name| name.as_str()) == expected.alias
                }
                (
                    SourceBody::Derived { query, .. },
                    TableSource::Derived {
                        query: actual,
                        alias,
                    },
                ) => actual == query && alias.as_ref().map(|name| name.as_str()) == expected.alias,
                _ => false,
            };
            if !matches {
                return Err(E::InvalidMetadata);
            }
        }
        Body::Join {
            left,
            right,
            kind,
            conditions,
        } => {
            // Unaliased parentheses preserve join topology but are normalized
            // out of the owned syntax tree by the parser.
            let TableSource::Join {
                left: actual_left,
                right: actual_right,
                kind: actual_kind,
                condition,
            } = actual
            else {
                return Err(E::InvalidMetadata);
            };
            if actual_kind != kind || !conditions::matches(conditions, condition, context)? {
                return Err(E::InvalidMetadata);
            }
            verify(forest, *left, actual_left, context)?;
            verify(forest, *right, actual_right, context)?;
        }
    }
    Ok(())
}
