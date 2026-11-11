//! Closed scalar predicate retrieval shared by UPDATE and DELETE.
pub(super) mod syntax;
use super::Binder;
use crate::{
    QueryError,
    plan::{Expr, ReadNode, ReadPlan, RelationNode},
};

pub(super) fn admit(expression: &Expr, binder: &mut Binder<'_>) -> Result<(), QueryError> {
    binder.work()?;
    match expression {
        Expr::Parameter(_)
        | Expr::Literal(_)
        | Expr::NumericLiteral(_)
        | Expr::DateLiteral(..)
        | Expr::Field { .. } => Ok(()),
        Expr::Unary(_, value)
        | Expr::IsNull(value, _)
        | Expr::Convert(_, value)
        | Expr::DateCall(_, value) => admit(value, binder),
        Expr::Binary(_, left, right) => {
            admit(left, binder)?;
            admit(right, binder)
        }
        Expr::Between(value, first, last, _) | Expr::IIf(value, first, last) => {
            admit(value, binder)?;
            admit(first, binder)?;
            admit(last, binder)
        }
        Expr::InList(value, values, _) => {
            admit(value, binder)?;
            for value in values {
                admit(value, binder)?;
            }
            Ok(())
        }
        Expr::Nz(value, replacement) => {
            admit(value, binder)?;
            if let Some(value) = replacement {
                admit(value, binder)?;
            }
            Ok(())
        }
        // Binder::mutation closes callable admission before this structural walk.
        Expr::TextCall(_, values)
        | Expr::MathCall(_, values)
        | Expr::ConstructorCall(_, values) => {
            for value in values {
                admit(value, binder)?;
            }
            Ok(())
        }
        Expr::Exists(read) => admit_read(read, binder),
        Expr::InQuery(value, read, _) => {
            admit(value, binder)?;
            admit_read(read, binder)
        }
        _ => Err(QueryError::Unsupported),
    }
}

fn admit_read(read: &ReadPlan, binder: &mut Binder<'_>) -> Result<(), QueryError> {
    binder.work()?;
    let ReadNode::Select(select) = &read.node else {
        return Err(QueryError::Unsupported);
    };
    if read.distinct
        || read.top.is_some()
        || !read.sort.is_empty()
        || select.aggregate
        || !select.group.is_empty()
        || select.having.is_some()
        || !select.record_keys.is_empty()
        || select.parent.is_some()
        || !read.recordset_schemas.is_empty()
    {
        return Err(QueryError::Unsupported);
    }
    let RelationNode::Table {
        namespace,
        multivalues,
        members,
        recordsets,
        ..
    } = &select.source.node
    else {
        return Err(QueryError::Unsupported);
    };
    if *namespace != crate::external_sources::SourceNamespace::Root
        || !multivalues.is_empty()
        || !members.is_empty()
        || !recordsets.is_empty()
    {
        return Err(QueryError::Unsupported);
    }
    for value in &select.projection {
        admit(value, binder)?;
    }
    if let Some(value) = &select.filter {
        admit(value, binder)?;
    }
    Ok(())
}
